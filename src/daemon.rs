//! Daemon
//!
//! Coordinator daemon run implementation

use bitcoin::util::hash::{HexError, Sha256dHash};

use crate::error::{CError, Result};
use crate::service::{MockService, Service};

/// Run method
pub fn run() -> Result<()> {
    println!("Running Coordinator daemon!");

    let genesis_hash =
        Sha256dHash::from_hex("73902d2a365fff2724e26d975148124268ec6a84991016683817ea2c973b199b")
            .unwrap();

    let service = MockService {};
    let service_req = service.get_request(&genesis_hash)?;
    match service_req {
        Some(req) => {
            println! {"Received request: {:?}", req};
            let service_bids = service.get_request_bids(&genesis_hash)?;
            if let Some(bids) = service_bids {
                println! {"and bids: {:?}", bids}
            }
        }
        _ => return Err(CError::Service("No requests found")),
    }
    Ok(())
}
