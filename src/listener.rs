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

/// Messsage type for challenge proofs sent by guardnodes
#[derive(Debug)]
pub struct ChallengeSig {
    /// Challenge (transaction id) hash
    pub hash: Sha256dHash,
    /// Pubkey used to generate challenge signature
    pub pubkey: PublicKey,
    /// Challenge signature for hash and pubkey
    pub sig: Signature,
}

impl ChallengeSig {
    /// Parse serde json value into ChallengeSig
    pub fn from_json(val: Value) -> Result<ChallengeSig> {
        let hash = Sha256dHash::from_hex(val["hash"].as_str().unwrap_or(""))?;
        let pubkey =
            PublicKey::from_slice(&Vec::<u8>::from_hex(val["pubkey"].as_str().unwrap_or(""))?)?;
        let sig = Signature::from_der(&Vec::<u8>::from_hex(val["sig"].as_str().unwrap_or(""))?)?;
        Ok(ChallengeSig { hash, pubkey, sig })
    }

    /// Verify that the challenge signature is valid using ecdsa tools
    fn verify(challenge_sig: ChallengeSig) -> Result<()> {
        let secp = Secp256k1::new();
        secp.verify(
            &Message::from_slice(&serialize(&challenge_sig.hash)).unwrap(),
            &challenge_sig.sig,
            &challenge_sig.pubkey,
        )?;
        Ok(())
    }
}

/// Handle challengeproof POST request
fn handle_challengeproof(
    req: Request<Body>,
    challenge: Arc<Mutex<ChallengeState>>,
    _challenge_resp: &Sender<ChallengeResponse>,
) -> Box<Future<Item = Response<Body>, Error = hyper::Error> + Send> {
    let resp = req.into_body().concat2().map(move |body| {
        // parse request body
        match serde_json::from_slice::<Value>(body.as_ref()) {
            // parse json from body
            Ok(obj) => match ChallengeSig::from_json(obj) {
                // parse challenge sig from json
                Ok(sig) => {
                    let latest = challenge.lock().unwrap().latest_challenge;
                    match latest {
                        // match active challenge hash with request
                        Some(h) => {
                            if sig.hash != h {
                                return response(
                                    StatusCode::BAD_REQUEST,
                                    format!("bad-hash: {:?}", sig.hash),
                                );
                            }
                        }
                        None => {
                            return response(
                                StatusCode::BAD_REQUEST,
                                format!("no active challenge"),
                            )
                        }
                    }

                    // After checking active challenge, verify challenge sig
                    if let Err(e) = ChallengeSig::verify(sig) {
                        return response(StatusCode::BAD_REQUEST, format!("bad-sig: {:?}", e));
                    }
                    response(StatusCode::OK, "Success".to_string())
                }
                Err(e) => response(
                    StatusCode::BAD_REQUEST,
                    format!("bad-challenge-data: {:?}", e),
                ),
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
    challenge_resp: &Sender<ChallengeResponse>,
) -> Box<Future<Item = Response<Body>, Error = hyper::Error> + Send> {
    let resp = match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => response(
            StatusCode::OK,
            "Challenge proof should be POSTed to /challengeproof".to_string(),
        ),

        (&Method::POST, "/challengeproof") => {
            return handle_challengeproof(req, challenge.clone(), challenge_resp);
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
        service_fn(move |req: Request<Body>| handle(req, challenge.clone(), &challenge_resp))
    };

    let server = Server::bind(&addr)
        .serve(listener_service)
        .with_graceful_shutdown(ch_recv)
        .map_err(|e| eprintln!("server error: {}", e));

    thread::spawn(move || {
        rt::run(server);
    })
}
