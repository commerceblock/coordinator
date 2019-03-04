//! Challenger
//!
//! Challenger entry point

use bitcoin::util::hash::{HexError, Sha256dHash};

use clientchain::ClientChain;
use error::{CError, Result};
use service::Service;

/// Run challenger main method
pub fn run_challenger<T: Service, K: ClientChain>(service: T, clientchain: K) -> Result<()> {
    info!("Running challenger!");

    let genesis_hash =
        Sha256dHash::from_hex("73902d2a365fff2724e26d975148124268ec6a84991016683817ea2c973b199b")
            .unwrap();

    let service_req = service.get_request(&genesis_hash)?;
    match service_req {
        Some(req) => {
            info! {"Received request: {:?}", req};
            let service_bids = service.get_request_bids(&genesis_hash)?;
            if let Some(bids) = service_bids {
                info! {"and bids: {:?}", bids}
            } else {
                warn! {"no bids found"}
            }
        }
        _ => return Err(CError::Coordinator("No requests found")),
    }
    Ok(())
}

// TODO
// Challenger struct ?
