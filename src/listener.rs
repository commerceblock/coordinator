//! Listener
//!
//! Listener interface and implementations

use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin::consensus::serialize;
use bitcoin::util::hash::{BitcoinHash, Sha256dHash};
use futures::future;
use futures::sync::oneshot;
use hyper::rt::{self, Future, Stream};
use hyper::service::{service_fn, service_fn_ok};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use secp256k1::{Message, PublicKey, Secp256k1, Signature};
use serde_json::{self, Value};

use crate::challenger::{ChallengeResponse, ChallengeState};
use crate::error::{CError, Result};

/// Messsage type for challenge proofs sent by guardnodes
pub struct ChallengeSig {
    hash: Sha256dHash,
    pubkey: PublicKey,
    sig: Signature,
}

/// Verify that the challenge signature is valid using ecdsa tools
fn verify_challenge_sig(challenge_sig: ChallengeSig) -> Result<()> {
    let secp = Secp256k1::new();

    match secp.verify(
        &Message::from_slice(&serialize(&challenge_sig.hash)).unwrap(),
        &challenge_sig.sig,
        &challenge_sig.pubkey,
    ) {
        Ok(_) => Ok(()),
        Err(_) => Err(CError::Coordinator("verify_challenge_sig failed")),
    }
}

/// Handle listener service requests
fn handle(
    req: Request<Body>,
    _challenge: &Arc<Mutex<ChallengeState>>,
    _challenge_resp: &Sender<ChallengeResponse>,
) -> Box<Future<Item = Response<Body>, Error = hyper::Error> + Send> {
    let mut response = Response::new(Body::empty());

    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => {
            *response.body_mut() =
                Body::from("Challenge proof should be POSTed to /challengeproof");
        }

        (&Method::POST, "/challengeproof") => {
            let resp = req.into_body().concat2().map(move |body| {
                match serde_json::from_slice::<Value>(body.as_ref()) {
                    Ok(obj) => {
                        info!("{:?}", obj);
                        Response::new(Body::from("Success"))
                    }
                    Err(e) => {
                        warn! {"serialization error: {:?}", e}
                        Response::new(Body::from("Invalid body"))
                    }
                }
            });
            return Box::new(resp);
        }

        _ => {
            *response.body_mut() = Body::from("Invalid request");
        }
    }

    Box::new(future::ok(response))
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
        service_fn(move |req: Request<Body>| handle(req, &challenge, &challenge_resp))
    };

    let server = Server::bind(&addr)
        .serve(listener_service)
        .with_graceful_shutdown(ch_recv)
        .map_err(|e| eprintln!("server error: {}", e));

    thread::spawn(move || {
        rt::run(server);
    })
}
