//! Challenger
//!
//! Methods and models for fetching, structuring and running challenge requests

use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::{thread, time};

use bitcoin::util::hash::{HexError, Sha256dHash};

use crate::clientchain::ClientChain;
use crate::coordinator;
use crate::error::{CError, Result};
use crate::request::{Bid, Request};
use crate::service::Service;

/// Run challenge for a specific request on the client chain
/// ...
/// ...
pub fn run_challenge_request<K: ClientChain>(
    clientchain: &K,
    challenge: &mut ChallengeRequest,
    verify_rx: Receiver<&'static str>,
) -> Result<()> {
    info! {"Running challenge for request: {:?}\n and bids: {:?}", challenge.request, challenge.bids};
    loop {
        let challenge_height = clientchain.get_blockheight()?;
        if challenge.request.end_blockheight < challenge_height as usize {
            break;
        }
        info! {"sending challenge (height: {})...", challenge_height}

        // send challenge
        //
        challenge.latest_challenge = Some(clientchain.send_challenge()?);

        // verify challenge
        //
        info! {"challenge verified. waiting for challenge proofs..."}

        // get challenge proofs
        //
        info! {"proof: {}", verify_rx.recv().unwrap()}

        thread::sleep(time::Duration::from_secs(1))
    }
    info! {"Request ended (endheight: {})", challenge.request.end_blockheight}
    Ok(())
}

/// Mainstains challenge state with information on
/// challenge requests and bids as well as the
/// latest challenge hash in the client chain
#[derive(Debug)]
pub struct ChallengeRequest {
    /// Service Request for issuing challenges
    request: Request,
    /// Request winning bids that respond to challenges
    bids: Vec<Bid>,
    /// Latest challenge txid hash in the client chain
    pub latest_challenge: Option<Sha256dHash>,
}

/// Check if request start height has been reached in order to initiate
/// challenging
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
pub fn fetch_challenge_request<T: Service, K: ClientChain>(
    service: &T,
    clientchain: &K,
    genesis: &Sha256dHash,
) -> Result<Option<ChallengeRequest>> {
    info!("Fetching challenge request!");
    match service.get_request(&genesis)? {
        Some(req) => {
            let height = clientchain.get_blockheight()?;
            if check_request(&req, height) {
                let bids = get_request_bids(&req, service)?;
                return Ok(Some(ChallengeRequest {
                    request: req,
                    bids: bids,
                    latest_challenge: None,
                }));
            } else {
                info! {"Request (startheight: {}) not ready for current height: {}", req.start_blockheight, height}
            }
        }
        _ => {
            warn! {"No request found for genesis: {}.", genesis}
        }
    }
    Ok(None)
}
