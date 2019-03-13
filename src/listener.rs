//! Listener
//!
//! Listener interface and implementations

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use bitcoin::consensus::serialize;
use bitcoin::util::hash::Sha256dHash;
use bitcoin_hashes::hex::FromHex;
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
pub struct ChallengeProof {
    /// Challenge (transaction id) hash
    pub hash: Sha256dHash,
    /// Challenge signature for hash and pubkey
    pub sig: Signature,
    /// Pubkey used to generate challenge signature
    pub bid: Bid,
}

impl ChallengeProof {
    /// Parse serde json value into ChallengeProof
    pub fn from_json(val: Value) -> Result<ChallengeProof> {
        let hash = Sha256dHash::from_hex(val["hash"].as_str().unwrap_or(""))?;
        let txid = Sha256dHash::from_hex(val["txid"].as_str().unwrap_or(""))?;
        let pubkey =
            PublicKey::from_slice(&Vec::<u8>::from_hex(val["pubkey"].as_str().unwrap_or(""))?)?;
        let sig = Signature::from_der(&Vec::<u8>::from_hex(val["sig"].as_str().unwrap_or(""))?)?;
        Ok(ChallengeProof {
            hash,
            sig,
            bid: Bid { txid, pubkey },
        })
    }

    /// Verify that the challenge signature is valid using ecdsa tools
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

// Future hyper return type for listener server responses
type BoxFut = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;

/// Handle challengeproof POST request
fn handle_challengeproof(
    req: Request<Body>,
    challenge: Arc<Mutex<ChallengeState>>,
    challenge_resp: Sender<ChallengeResponse>,
) -> BoxFut {
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
                            return response(StatusCode::BAD_REQUEST, "bad-bid".to_string());
                        }
                        // drop lock immediately
                        std::mem::drop(challenge_lock);
                        // check challenge proof hash is correct
                        if proof.hash != h {
                            return response(StatusCode::BAD_REQUEST, "bad-hash".to_string());
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
    Box::new(resp)
}

/// Handle listener service requests
fn handle(
    req: Request<Body>,
    challenge: Arc<Mutex<ChallengeState>>,
    challenge_resp: Sender<ChallengeResponse>,
) -> BoxFut {
    let resp = match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => response(
            StatusCode::OK,
            "Challenge proof should be POSTed to /challengeproof".to_string(),
        ),

        (&Method::POST, "/challengeproof") => {
            return handle_challengeproof(req, challenge, challenge_resp);
        }

        _ => response(
            StatusCode::NOT_FOUND,
            format!("Invalid request {}", req.uri().path()),
        ),
    };

    Box::new(future::ok(resp))
}

/// Create hyper response from status code and message
fn response(status: StatusCode, message: String) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::from(format!("{}", message)))
        .unwrap()
}

/// Run listener service
pub fn run_listener(
    challenge: Arc<Mutex<ChallengeState>>,
    ch_resp: Sender<ChallengeResponse>,
    ch_recv: oneshot::Receiver<()>,
) -> thread::JoinHandle<()> {
    let addr = ([127, 0, 0, 1], 9999).into();

    let listener_service = move || {
        let challenge = Arc::clone(&challenge);
        let challenge_resp = ch_resp.clone();
        service_fn(move |req: Request<Body>| handle(req, challenge.clone(), challenge_resp.clone()))
    };

    let server = Server::bind(&addr)
        .serve(listener_service)
        .with_graceful_shutdown(ch_recv)
        .map_err(|e| eprintln!("server error: {}", e));

    thread::spawn(move || {
        rt::run(server);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin_hashes::hex::ToHex;
    use secp256k1::SecretKey;
    use std::sync::mpsc::{channel, Receiver, TryRecvError};

    use crate::service::{MockService, Service};

    /// Generate dummy hash for tests
    fn gen_dummy_hash(i: u8) -> Sha256dHash {
        Sha256dHash::from(&[i as u8; 32] as &[u8])
    }

    /// Geberate dummy challenge state
    fn gen_challenge_state(
        request_hash: &Sha256dHash,
        challenge_hash: &Sha256dHash,
    ) -> ChallengeState {
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
    fn handle_challengeproof_test() {
        let (resp_tx, resp_rx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) =
            channel();

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
        let _ =
            handle_challengeproof(request, challenge_state.clone(), resp_tx.clone())
                .map(|res| {
                    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
                    res.into_body().concat2().map(|chunk| {
                    assert!(String::from_utf8_lossy(&chunk).contains("no-active-challenge"));
                }).wait()
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
        let sig = secp.sign(
            &Message::from_slice(&serialize(&chl_hash)).unwrap(),
            &secret_key,
        );
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
