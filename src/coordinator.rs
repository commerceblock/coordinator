//! Coordinator
//!
//! Coordinator entry point for spawning all components

use std::rc::Rc;
use std::{thread, time};

use bitcoin::util::hash::{HexError, Sha256dHash};

use crate::clientchain::MockClientChain;
use crate::error::{CError, Result};
use crate::service::{MockService, Service};

/// Run coordinator main method
/// Currently using mock interfaces until ocean rpcs are finished
pub fn run() -> Result<()> {
    info!("Running coordinator!");

    let service = MockService {};
    let clientchain = MockClientChain {};

    // hardcoded genesis hash for now
    // TODO: from config
    let genesis_hash =
        Sha256dHash::from_hex("73902d2a365fff2724e26d975148124268ec6a84991016683817ea2c973b199b")
            .unwrap();

    loop {
        if let Some(mut challenge_state) =
            ::challenge::fetch_challenge_request(&service, &clientchain, &genesis_hash)?
        {
            ::challenge::run_challenge_request(&clientchain, &mut challenge_state)?;
            info!("Latest challenge: {:?}", challenge_state.latest_challenge);
            break;
        }
        info! {"Sleeping for 5 sec..."}
        thread::sleep(time::Duration::from_secs(5))
    }
    Ok(())
}
