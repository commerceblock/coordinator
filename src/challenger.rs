//! Challenger
//!
//! Methods and models for fetching, structuring and running challenge requests

use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin::util::hash::{HexError, Sha256dHash};

use crate::clientchain::ClientChain;
use crate::error::{CError, Result};
use crate::request::{Bid, Request};
use crate::service::Service;

static NUM_VERIFY_ATTEMPTS: u8 = 5;

/// Attempts to verify that a challenge has been included in the client chain
/// Method tries a fixed number of attempts NUM_VERIFY_ATTEMPTS with variable
// delay time between these to allow easy configuration
pub fn verify_challenge<K: ClientChain>(
    hash: &Sha256dHash,
    clientchain: &K,
    attempt_delay: time::Duration,
) -> Result<bool> {
    info! {"verifying challenge hash: {}", hash}
    for i in 0..NUM_VERIFY_ATTEMPTS {
        if clientchain.verify_challenge(&hash)? {
            info! {"challenge verified"}
            return Ok(true);
        }
        warn! {"attempt {} failed", i+1}
        if i + 1 == NUM_VERIFY_ATTEMPTS {
            break;
        }
        info! {"sleeping for {}sec...", attempt_delay.as_secs()}
        thread::sleep(attempt_delay)
    }
    Ok(false)
}

/// Get responses to the challenge by reading data from the channel receiver
/// Channel is read for a configurable duration and then the method returns
/// all the responses that have been received
pub fn get_challenge_responses(
    verify_rx: &Receiver<ChallengeResponse>,
    get_duration: time::Duration,
) -> Result<Vec<ChallengeResponse>> {
    let mut responses = vec![];

    let (dur_tx, dur_rx) = channel();
    let _ = thread::spawn(move || {
        thread::sleep(get_duration);
        dur_tx.send("tick").unwrap();
    });

    let mut time_to_break = false;
    while time_to_break == false {
        match verify_rx.try_recv() {
            Ok(resp) => responses.push(resp),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(CError::Coordinator(
                    "Challenge response receiver disconnected",
                ))
            }
        }
        let _ = dur_rx.try_recv().map(|_| time_to_break = true);
    }

    Ok(responses)
}

/// Run challenge for a specific request on the client chain
/// ...
/// ...
pub fn run_challenge_request<K: ClientChain>(
    clientchain: &K,
    challenge_state: Arc<Mutex<ChallengeState>>,
    verify_rx: Receiver<ChallengeResponse>,
) -> Result<()> {
    info! {"Running challenge request: {:?}", challenge_state.lock().unwrap().request};
    loop {
        let challenge_height = clientchain.get_blockheight()?;
        info! {"client chain height: {}", challenge_height}
        if challenge_state.lock().unwrap().request.end_blockheight < challenge_height as usize {
            break;
        }

        // send challenge
        info! {"sending challenge..."}
        let challenge_hash = clientchain.send_challenge()?;
        challenge_state.lock().unwrap().latest_challenge = Some(challenge_hash);

        // verify challenge
        if !verify_challenge(&challenge_hash, clientchain, time::Duration::from_secs(1))? {
            continue;
        }

        // get challenge proofs
        info! {"responses : {:?}", get_challenge_responses(&verify_rx, time::Duration::from_secs(1))?}

        thread::sleep(time::Duration::from_secs(1))
    }
    info! {"Challenge request ended"}
    Ok(())
}

/// Tuple struct to store a verified challenge response
/// for a winning bid on a specific challenge hash
#[derive(Debug)]
pub struct ChallengeResponse(pub Sha256dHash, pub Bid);

/// Mainstains challenge state with information on
/// challenge requests and bids as well as the
/// latest challenge hash in the client chain
#[derive(Debug)]
pub struct ChallengeState {
    /// Service Request for issuing challenges
    pub request: Request,
    /// Request winning bids that respond to challenges
    pub bids: Vec<Bid>,
    /// Latest challenge txid hash in the client chain
    pub latest_challenge: Option<Sha256dHash>,
}

/// Check if request start height has been reached in order to initiate
/// challenging.
fn check_request(request: &Request, height: u64) -> bool {
    return if request.start_blockheight <= height as usize {
        true
    } else {
        false
    };
}

/// Attempt to fetch the winnings bids for a request in the service chain
fn get_request_bids<T: Service>(request: &Request, service: &T) -> Result<Vec<Bid>> {
    match service.get_request_bids(&request.genesis_blockhash)? {
        Some(bids) => return Ok(bids),
        _ => Err(CError::Coordinator("No bids found")),
    }
}

/// Fetch next challenge state given a request and bids in the service chain
/// A challenge is fetched only when a request exists and the required
/// starting blockheight has been reached in the corresponding client chain
pub fn fetch_next<T: Service, K: ClientChain>(
    service: &T,
    clientchain: &K,
    genesis: &Sha256dHash,
) -> Result<Option<ChallengeState>> {
    info!("Fetching challenge request!");
    match service.get_request(&genesis)? {
        Some(req) => {
            let height = clientchain.get_blockheight()?;
            if check_request(&req, height) {
                let bids = get_request_bids(&req, service)?;
                return Ok(Some(ChallengeState {
                    request: req,
                    bids: bids,
                    latest_challenge: None,
                }));
            } else {
                warn! {"Request (startheight: {}) not ready for current height: {}", req.start_blockheight, height}
            }
        }
        _ => {
            warn! {"No request found for genesis: {}.", genesis}
        }
    }
    Ok(None)
}
