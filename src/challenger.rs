//! Challenger
//!
//! Challenger entry point

use bitcoin::util::hash::{HexError, Sha256dHash};
use std::{thread, time};

use crate::clientchain::ClientChain;
use crate::error::{CError, Result};
use crate::request::Request;
use crate::service::Service;

/// Run challenge for a specific request
pub fn run_request_challenge<T: Service, K: ClientChain>(
    service: &T,
    clientchain: &K,
    genesis_hash: &Sha256dHash,
    req: Request,
) -> Result<()> {
    info! {"Initiating request challenge: {:?}", req};
    let service_bids = service.get_request_bids(&genesis_hash)?;
    match service_bids {
        Some(bids) => {
            info! {"Bids: {:?}", bids}
            loop {
                let challenge_height = clientchain.get_blockheight()?;
                if req.end_blockheight < challenge_height as usize {
                    break;
                }
                info! {"sending challenge (height: {})...", challenge_height}

                // send challenge
                //

                // verify challenge
                //

                thread::sleep(time::Duration::from_secs(1))
            }
            info! {"Request ended (endheight: {})", req.end_blockheight}
            Ok(())
        }
        _ => Err(CError::Coordinator("No bids found")),
    }
}

/// Check if a request is ready to initiate challenging
pub fn check_request(request: &Request, height: u64) -> Result<bool> {
    if request.start_blockheight <= height as usize {
        return Ok(true);
    }
    Ok(false)
}

/// Run challenger main method
pub fn run_challenger<T: Service, K: ClientChain>(service: &T, clientchain: &K) -> Result<()> {
    // hardcoded genesis hash for now
    // TODO: from config
    let genesis_hash =
        Sha256dHash::from_hex("73902d2a365fff2724e26d975148124268ec6a84991016683817ea2c973b199b")
            .unwrap();

    info!("Challenger reporting!");
    loop {
        let get_req = service.get_request(&genesis_hash)?;
        match get_req {
            Some(req) => {
                let height = clientchain.get_blockheight()?;
                if check_request(&req, height)? {
                    run_request_challenge(service, clientchain, &genesis_hash, req)?;
                    break;
                } else {
                    info! {"Request (startheight: {}) not ready for current height: {}", req.start_blockheight, height}
                }
            }
            _ => {
                warn! {"No request found. Sleeping for 60 secs..."}
                thread::sleep(time::Duration::from_secs(60))
            }
        }
    }
    Ok(())
}
