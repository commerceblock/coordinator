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

/// Handle challengeproof POST request
fn handle_challengeproof(
    req: Request<Body>,
    challenge: Arc<Mutex<ChallengeState>>,
    challenge_resp: Sender<ChallengeResponse>,
) -> Box<Future<Item = Response<Body>, Error = hyper::Error> + Send> {
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
                            return response(StatusCode::BAD_REQUEST, format!("bad-sig: {:?}", e));
                        }
                        // send successful response to challenger
                        challenge_resp
                            .send(ChallengeResponse(proof.hash, proof.bid.clone()))
                            .unwrap();
                        return response(StatusCode::OK, String::new());
                    }
                    response(StatusCode::BAD_REQUEST, format!("no-active-challenge"))
                }
                Err(e) => response(StatusCode::BAD_REQUEST, format!("bad-proof-data: {:?}", e)),
            },
            Err(e) => response(StatusCode::BAD_REQUEST, format!("bad-json-data: {:?}", e)),
        }
    });
    Box::new(resp)
}

/// Handle listener service requests
fn handle(
    req: Request<Body>,
    challenge: Arc<Mutex<ChallengeState>>,
    challenge_resp: Sender<ChallengeResponse>,
) -> Box<Future<Item = Response<Body>, Error = hyper::Error> + Send> {
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
            format!("Invalid request {:?}", req.uri().path()),
        ),
    };

    Box::new(future::ok(resp))
}

/// Create hyper response from status code and message
fn response(status: StatusCode, message: String) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::from(format!("{:?}", message)))
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
