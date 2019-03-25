//! Listener
//!
//! Listener interface and implementations

use std::net::ToSocketAddrs;
use std::str::FromStr;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use bitcoin::consensus::serialize;
use bitcoin_hashes::{hex::FromHex, sha256d};
use futures::future;
use futures::sync::oneshot;
use hyper::rt::{self, Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use secp256k1::{Message, PublicKey, Secp256k1, Signature};
use serde_json::{self, Value};

use crate::challenger::{ChallengeResponse, ChallengeState};
use crate::error::Result;
use crate::request::Bid;

/// Messsage type for challenge proofs sent by guardnodes
#[derive(Debug)]
struct ChallengeProof {
    /// Challenge (transaction id) hash
    hash: sha256d::Hash,
    /// Challenge signature for hash and pubkey
    sig: Signature,
    /// Pubkey used to generate challenge signature
    bid: Bid,
}

impl ChallengeProof {
    /// Parse serde json value into ChallengeProof struct result
    fn from_json(val: Value) -> Result<ChallengeProof> {
        let hash = sha256d::Hash::from_hex(val["hash"].as_str().unwrap_or(""))?;
        let txid = sha256d::Hash::from_hex(val["txid"].as_str().unwrap_or(""))?;
        let pubkey = PublicKey::from_str(val["pubkey"].as_str().unwrap_or(""))?;
        let sig = Signature::from_der(&Vec::<u8>::from_hex(val["sig"].as_str().unwrap_or(""))?)?;
        Ok(ChallengeProof {
            hash,
            sig,
            bid: Bid { txid, pubkey },
        })
    }

    /// Verify the challenge proof signature using the pubkey and challenge hash
    fn verify(challenge_proof: &ChallengeProof) -> Result<()> {
        let secp = Secp256k1::new();
        secp.verify(
            &Message::from_slice(&serialize(&challenge_proof.hash)).unwrap(),
            &challenge_proof.sig,
            &challenge_proof.bid.pubkey,
        )?;
        Ok(())
    }
}

/// Handle the POST request /challengeproof. Validate body is in json format,
/// parse this into a ChallengeProof struct and then verify that there is an
/// active challenge, that the proof bid exists and that the sig is correct.
/// Successful responses are pushed to the challenge response channel for the
/// challenger to receive
fn handle_challengeproof(
    req: Request<Body>,
    challenge: Arc<Mutex<ChallengeState>>,
    challenge_resp: Sender<ChallengeResponse>,
) -> impl Future<Item = Response<Body>, Error = hyper::Error> + Send {
    let resp = req.into_body().concat2().map(move |body| {
        // parse request body
        match serde_json::from_slice::<Value>(body.as_ref()) {
            // parse json from body
            Ok(obj) => match ChallengeProof::from_json(obj) {
                // parse challenge proof from json
                Ok(proof) => {
                    // check for an active challenge
                    let challenge_lock = challenge.lock().unwrap();
                    if let Some(h) = challenge_lock.latest_challenge {
                        // check challenge proof bid exists
                        if !challenge_lock.bids.contains(&proof.bid) {
                            return response(StatusCode::BAD_REQUEST, "bad-bid".to_owned());
                        }
                        // drop lock immediately
                        std::mem::drop(challenge_lock);
                        // check challenge proof hash is correct
                        if proof.hash != h {
                            return response(StatusCode::BAD_REQUEST, "bad-hash".to_owned());
                        }
                        // check challenge proof sig is correct
                        if let Err(e) = ChallengeProof::verify(&proof) {
                            return response(StatusCode::BAD_REQUEST, format!("bad-sig: {}", e));
                        }
                        // send successful response to challenger
                        challenge_resp
                            .send(ChallengeResponse(proof.hash, proof.bid.clone()))
                            .unwrap();
                        return response(StatusCode::OK, String::new());
                    }
                    response(StatusCode::BAD_REQUEST, format!("no-active-challenge"))
                }
                Err(e) => response(StatusCode::BAD_REQUEST, format!("bad-proof-data: {}", e)),
            },
            Err(e) => response(StatusCode::BAD_REQUEST, format!("bad-json-data: {}", e)),
        }
    });
    resp
}

/// Handler for the listener server. Only allows requests to /
/// and to the /challengeproof POST uri for receiving challenges from guardnodes
fn handle(
    req: Request<Body>,
    challenge: Arc<Mutex<ChallengeState>>,
    challenge_resp: Sender<ChallengeResponse>,
) -> impl Future<Item = Response<Body>, Error = hyper::Error> + Send {
    let resp = match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => response(
            StatusCode::OK,
            "Challenge proof should be POSTed to /challengeproof".to_owned(),
        ),

        (&Method::POST, "/challengeproof") => {
            return future::Either::A(handle_challengeproof(req, challenge, challenge_resp));
        }

        _ => response(StatusCode::NOT_FOUND, format!("Invalid request {}", req.uri().path())),
    };

    future::Either::B(future::ok(resp))
}

/// Create hyper response from status code and message Body
fn response(status: StatusCode, message: String) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::from(format!("{}", message)))
        .unwrap()
}

/// Run the listener server that listens to a specified address for incoming
/// requests and passes these to handle(). The server runs in a new thread and
/// can be shutdown via a future oneshot channel receiver from the main method
/// of the coordinator
pub fn run_listener(
    listener_host: &String,
    challenge: Arc<Mutex<ChallengeState>>,
    ch_resp: Sender<ChallengeResponse>,
    ch_recv: oneshot::Receiver<()>,
) -> thread::JoinHandle<()> {
    let addr: Vec<_> = listener_host
        .to_socket_addrs()
        .expect("Unable to resolve domain")
        .collect();

    let listener_service = move || {
        let challenge = Arc::clone(&challenge);
        let challenge_resp = ch_resp.clone();
        service_fn(move |req: Request<Body>| handle(req, challenge.clone(), challenge_resp.clone()))
    };

    let server = Server::bind(&addr[0])
        .serve(listener_service)
        .with_graceful_shutdown(ch_recv)
        .map_err(|e| error!("server error: {}", e));

    thread::spawn(move || {
        rt::run(server);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc::{channel, Receiver, TryRecvError};

    use bitcoin_hashes::hex::ToHex;
    use bitcoin_hashes::Hash;
    use secp256k1::SecretKey;

    use crate::service::{MockService, Service};

    /// Generate dummy hash for tests
    fn gen_dummy_hash(i: u8) -> sha256d::Hash {
        sha256d::Hash::from_slice(&[i as u8; 32] as &[u8]).unwrap()
    }

    /// Geberate dummy challenge state
    fn gen_challenge_state(request_hash: &sha256d::Hash, challenge_hash: &sha256d::Hash) -> ChallengeState {
        let service = MockService::new();

        let request = service.get_request(&request_hash).unwrap().unwrap();
        let bids = service.get_request_bids(&request_hash).unwrap().unwrap();
        ChallengeState {
            request,
            bids,
            latest_challenge: Some(*challenge_hash),
        }
    }

    #[test]
    fn challengeproof_from_json_test() {
        // good data
        let data = r#"
        {
            "txid": "0000000000000000000000000000000000000000000000000000000000000000",
            "pubkey": "03356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d111",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }"#;
        let proof = ChallengeProof::from_json(serde_json::from_str::<Value>(data).unwrap());
        assert!(proof.is_ok());

        // bad txid
        let data = r#"
        {
            "txid": "",
            "pubkey": "03356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d111",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }"#;
        let proof = ChallengeProof::from_json(serde_json::from_str::<Value>(data).unwrap());
        assert!(proof.err().unwrap().to_string().contains("bitcoin hashes error"));

        // bad pubkey
        let data = r#"
        {
            "txid": "0000000000000000000000000000000000000000000000000000000000000000",
            "pubkey": "0356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d111",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }"#;
        let proof = ChallengeProof::from_json(serde_json::from_str::<Value>(data).unwrap());
        assert!(proof.err().unwrap().to_string().contains("secp256k1 error"));

        // bad hash
        let data = r#"
        {
            "txid": "0000000000000000000000000000000000000000000000000000000000000000",
            "pubkey": "03356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d111",
            "hash": "04040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }"#;
        let proof = ChallengeProof::from_json(serde_json::from_str::<Value>(data).unwrap());
        assert!(proof.err().unwrap().to_string().contains("bitcoin hashes error"));

        // bad sig
        let data = r#"
        {
            "txid": "0000000000000000000000000000000000000000000000000000000000000000",
            "pubkey": "03356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d111",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "4402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }"#;
        let proof = ChallengeProof::from_json(serde_json::from_str::<Value>(data).unwrap());
        assert!(proof.err().unwrap().to_string().contains("secp256k1 error"));
    }

    #[test]
    fn challengeproof_verify_test() {
        let chl_hash = gen_dummy_hash(11);
        let _challenge_state = gen_challenge_state(&gen_dummy_hash(3), &chl_hash);
        let bid_txid = _challenge_state.bids.iter().next().unwrap().txid;
        let bid_pubkey = _challenge_state.bids.iter().next().unwrap().pubkey;

        // verify good sig
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0xaa; 32]).unwrap();
        let sig = secp.sign(&Message::from_slice(&serialize(&chl_hash)).unwrap(), &secret_key);

        let proof = ChallengeProof {
            hash: chl_hash,
            sig: sig,
            bid: Bid {
                txid: bid_txid,
                pubkey: bid_pubkey,
            },
        };

        let verify = ChallengeProof::verify(&proof);
        assert!(verify.is_ok());

        // verify bad sig
        let secret_key = SecretKey::from_slice(&[0xbb; 32]).unwrap();
        let sig = secp.sign(&Message::from_slice(&serialize(&chl_hash)).unwrap(), &secret_key);

        let proof = ChallengeProof {
            hash: chl_hash,
            sig: sig,
            bid: Bid {
                txid: bid_txid,
                pubkey: bid_pubkey,
            },
        };

        let verify = ChallengeProof::verify(&proof);
        assert!(verify.err().unwrap().to_string().contains("secp256k1 error"));
    }

    #[test]
    fn handle_test() {
        let (resp_tx, resp_rx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

        let chl_hash = gen_dummy_hash(11);
        let _challenge_state = gen_challenge_state(&gen_dummy_hash(3), &chl_hash);
        let bid_txid = _challenge_state.bids.iter().next().unwrap().txid;
        let bid_pubkey = _challenge_state.bids.iter().next().unwrap().pubkey;
        let challenge_state = Arc::new(Mutex::new(_challenge_state));

        // Request get /
        let data = "";
        let request = Request::builder()
            .method("GET")
            .uri("/")
            .body(Body::from(data))
            .unwrap();
        let _ = handle(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::OK);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert_eq!(
                            "Challenge proof should be POSTed to /challengeproof",
                            String::from_utf8_lossy(&chunk)
                        );
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Request get /dummy
        let data = "";
        let request = Request::builder()
            .method("GET")
            .uri("/dummy")
            .body(Body::from(data))
            .unwrap();
        let _ = handle(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::NOT_FOUND);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert_eq!("Invalid request /dummy", String::from_utf8_lossy(&chunk));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Request post /dummy
        let data = "";
        let request = Request::builder()
            .method("POST")
            .uri("/dummy")
            .body(Body::from(data))
            .unwrap();
        let _ = handle(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::NOT_FOUND);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert_eq!("Invalid request /dummy", String::from_utf8_lossy(&chunk));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Request empty post /challengeproof
        let data = "";
        let request = Request::builder()
            .method("POST")
            .uri("/challengeproof")
            .body(Body::from(data))
            .unwrap();
        let _ = handle(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-json-data"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Request good post /challengeproof
        let secret_key = SecretKey::from_slice(&[0xaa; 32]).unwrap();
        let secp = Secp256k1::new();
        let sig = secp.sign(&Message::from_slice(&serialize(&chl_hash)).unwrap(), &secret_key);
        let data = format!(
            r#"
        {{
            "txid": "{}",
            "pubkey": "{}",
            "hash": "{}",
            "sig": "{}"
        }}"#,
            bid_txid,
            bid_pubkey,
            chl_hash,
            sig.serialize_der().to_hex()
        );
        let request = Request::builder()
            .method("POST")
            .uri("/challengeproof")
            .body(Body::from(data))
            .unwrap();
        let _ = handle(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::OK);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert_eq!("", String::from_utf8_lossy(&chunk));
                    })
                    .wait()
            })
            .wait();
        assert!(
            resp_rx.try_recv()
                == Ok(ChallengeResponse(
                    chl_hash,
                    Bid {
                        txid: bid_txid,
                        pubkey: bid_pubkey,
                    },
                ))
        ); // check receiver not empty
    }

    #[test]
    fn handle_challengeproof_test() {
        let (resp_tx, resp_rx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

        let chl_hash = gen_dummy_hash(8);
        let _challenge_state = gen_challenge_state(&gen_dummy_hash(1), &chl_hash);
        let bid_txid = _challenge_state.bids.iter().next().unwrap().txid;
        let bid_pubkey = _challenge_state.bids.iter().next().unwrap().pubkey;
        let challenge_state = Arc::new(Mutex::new(_challenge_state));

        // Request body data empty
        let data = "";
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-json-data"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Bad json data on request body (extra comma)
        let data = r#"
        {
            "txid": "1234567890000000000000000000000000000000000000000000000000000000",
        }"#;
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-json-data"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Missing proof data on request body
        let data = r#"
        {
            "txid": "1234567890000000000000000000000000000000000000000000000000000000"
        }"#;
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-proof-data"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Bad proof data on request body (invalid pubkey)
        let data = r#"
        {
            "txid": "1234567890000000000000000000000000000000000000000000000000000000",
            "pubkey": "3356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d2f3",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }"#;
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-proof-data"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // No active challenge (hash is None) so request rejected
        challenge_state.lock().unwrap().latest_challenge = None;
        let data = r#"
        {
            "txid": "0000000000000000000000000000000000000000000000000000000000000000",
            "pubkey": "03356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d111",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }"#;
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("no-active-challenge"));
                    })
                    .wait()
            })
            .wait();
        challenge_state.lock().unwrap().latest_challenge = Some(chl_hash);
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Invalid bid on request body (txid does not exist)
        let data = r#"
        {
            "txid": "0000000000000000000000000000000000000000000000000000000000000000",
            "pubkey": "03356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d111",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }"#;
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-bid"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Invalid bid on request body (pubkey does not exist)
        let data = format!(r#"
        {{
            "txid": "{}",
            "pubkey": "03356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d111",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }}"#, bid_txid);
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-bid"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Request send for an invalid / out of date challenge hash
        let data = format!(r#"
        {{
            "txid": "{}",
            "pubkey": "{}",
            "hash": "0404040404040404040404040404040404040404040404040404040404040404",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }}"#, bid_txid, bid_pubkey);
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-hash"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Request sent an invalid sig for the correct bid and challenge hash
        let data = format!(r#"
        {{
            "txid": "{}",
            "pubkey": "{}",
            "hash": "{}",
            "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
        }}"#, bid_txid, bid_pubkey, chl_hash);
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk).contains("bad-sig"));
                    })
                    .wait()
            })
            .wait();
        assert!(resp_rx.try_recv() == Err(TryRecvError::Empty)); // check receiver empty

        // Correct sig sent in the request body for bid and active challenge
        let secret_key = SecretKey::from_slice(&[0xaa; 32]).unwrap();
        let secp = Secp256k1::new();
        let sig = secp.sign(&Message::from_slice(&serialize(&chl_hash)).unwrap(), &secret_key);
        let data = format!(
            r#"
        {{
            "txid": "{}",
            "pubkey": "{}",
            "hash": "{}",
            "sig": "{}"
        }}"#,
            bid_txid,
            bid_pubkey,
            chl_hash,
            sig.serialize_der().to_hex()
        );
        let request = Request::new(Body::from(data));
        let _ = handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
            .map(|res| {
                assert_eq!(res.status(), StatusCode::OK);
                res.into_body()
                    .concat2()
                    .map(|chunk| {
                        assert!(String::from_utf8_lossy(&chunk) == "");
                    })
                    .wait()
            })
            .wait();
        assert!(
            resp_rx.try_recv()
                == Ok(ChallengeResponse(
                    chl_hash,
                    Bid {
                        txid: bid_txid,
                        pubkey: bid_pubkey,
                    },
                ))
        ); // check receiver not empty
    }
}
