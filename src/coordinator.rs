//! Coordinator
//!
//! Coordinator entry point for spawning all components

use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin::util::hash::{HexError, Sha256dHash};

use crate::challenger::{ChallengeRequest, ChallengeVerify};
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
        if let Some(challenge) =
            ::challenger::fetch_challenge_request(&service, &clientchain, &genesis_hash)?
        {
            let mut shared_challenge = Arc::new(Mutex::new(challenge));

            let (thread_tx, thread_rx) = channel();
            let (verify_tx, verify_rx): (Sender<ChallengeVerify>, Receiver<ChallengeVerify>) =
                channel();

            let verify_handle = run_verify(shared_challenge.clone(), verify_tx, thread_rx);

            ::challenger::run_challenge_request(&clientchain, shared_challenge.clone(), verify_rx)?;

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
pub fn run_verify(
    challenge: Arc<Mutex<ChallengeRequest>>,
    vtx: Sender<ChallengeVerify>,
    trx: Receiver<()>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        match trx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                info!("Terminating verify");
                break;
            }
            Err(TryRecvError::Empty) => {}
        }

        // get immutable lock to avoid changing any data
        let challenge_lock = challenge.lock().unwrap();

        if let Some(latest) = challenge_lock.latest_challenge {
            vtx.send(ChallengeVerify {
                challenge: latest,
                bid: challenge_lock.bids[0].clone(),
            })
            .unwrap();
        }
        std::mem::drop(challenge_lock);

        thread::sleep(time::Duration::from_secs(1))
    })
}
