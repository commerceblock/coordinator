//! Coordinator
//!
//! Coordinator entry point for spawning all components

use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin::util::hash::{HexError, Sha256dHash};

use crate::challenger::{ChallengeResponse, ChallengeState};
use crate::clientchain::MockClientChain;
use crate::error::{CError, Result};
use crate::listener::{Listener, MockListener};
use crate::service::MockService;
use crate::storage::{MockStorage, Storage};

/// Run coordinator main method
/// Currently using mock interfaces until ocean rpcs are finished
pub fn run() -> Result<()> {
    info!("Running coordinator!");

    let service = MockService::new();
    let clientchain = MockClientChain::new();
    let listener = MockListener {};
    let storage = MockStorage::new();

    // hardcoded genesis hash for now
    // TODO: from config
    let genesis_hash =
        Sha256dHash::from_hex("73902d2a365fff2724e26d975148124268ec6a84991016683817ea2c973b199b")
            .unwrap();

    loop {
        if let Some(challenge) = ::challenger::fetch_next(&service, &clientchain, &genesis_hash)? {
            storage.save_challenge_state(challenge.clone())?;

            let mut shared_challenge = Arc::new(Mutex::new(challenge));

            let (thread_tx, thread_rx) = channel();
            let (verify_tx, verify_rx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) =
                channel();

            let verify_handle = listener.do_work(shared_challenge.clone(), verify_tx, thread_rx);

            ::challenger::run_challenge_request(
                &clientchain,
                shared_challenge.clone(),
                &verify_rx,
                &storage,
                time::Duration::from_secs(1),
                time::Duration::from_secs(1),
            )?;

            println! {"***** Responses *****"}
            for resp in storage.challenge_responses.borrow().iter() {
                println! {"{:?}", resp}
            }
            thread_tx.send(()).expect("thread_tx send failed");
            verify_handle.join().expect("verify_handle join failed");
            break;
        }
        info! {"Sleeping for 1 sec..."}
        thread::sleep(time::Duration::from_secs(1))
    }
    Ok(())
}
