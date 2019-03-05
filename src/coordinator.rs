//! Coordinator
//!
//! Coordinator entry point for spawning all components

use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
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
            ::challenger::fetch_challenge_request(&service, &clientchain, &genesis_hash)?
        {
            let (thread_tx, thread_rx) = channel();
            let (verify_tx, verify_rx) = channel();
            let verify_handle = run_verify(verify_tx, thread_rx);

            ::challenger::run_challenge_request(&clientchain, &mut challenge_state, verify_rx)?;

            thread_tx.send(()).expect("thread_tx send failed");
            verify_handle.join().expect("verify_handle join failed");
            break;
        }
        info! {"Sleeping for 5 sec..."}
        thread::sleep(time::Duration::from_secs(5))
    }
    Ok(())
}

/// Run challenge verifier method
/// Currently mock replies to challenge requests
pub fn run_verify(vtx: Sender<&'static str>, trx: Receiver<()>) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        match trx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                info!("Terminating run_verify.");
                break;
            }
            Err(TryRecvError::Empty) => {}
        }
        vtx.send("test").unwrap();
        thread::sleep(time::Duration::from_secs(1))
    })
}
