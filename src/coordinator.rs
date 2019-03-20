//! Coordinator
//!
//! Coordinator entry point for spawning all components

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin_hashes::{hex::FromHex, sha256d};
use futures::sync::oneshot;

use crate::challenger::ChallengeResponse;
use crate::clientchain::RpcClientChain;
use crate::error::Result;
use crate::service::MockService;
use crate::storage::{MockStorage, Storage};

/// Run coordinator main method
/// Currently using mock interfaces until ocean rpcs are finished
pub fn run() -> Result<()> {
    info!("Running coordinator!");

    let service = MockService::new();
    let clientchain = RpcClientChain::new(
        String::from("http://127.0.0.1:5555"),
        Some(String::from("user1")),
        Some(String::from("password1")),
        "CHALLENGE",
    )?;
    let storage = MockStorage::new();

    // hardcoded genesis hash for now
    // TODO: from config
    let genesis_hash =
        sha256d::Hash::from_hex("73902d2a365fff2724e26d975148124268ec6a84991016683817ea2c973b199b").unwrap();

    loop {
        if let Some(challenge) = ::challenger::fetch_next(&service, &clientchain, &genesis_hash)? {
            storage.save_challenge_state(&challenge)?;

            let mut shared_challenge = Arc::new(Mutex::new(challenge));

            let (thread_tx, thread_rx) = oneshot::channel();

            let (verify_tx, verify_rx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

            let verify_handle = ::listener::run_listener(shared_challenge.clone(), verify_tx, thread_rx);

            ::challenger::run_challenge_request(
                &clientchain,
                shared_challenge.clone(),
                &verify_rx,
                &storage,
                time::Duration::from_secs(150),
                time::Duration::from_secs(60),
            )?;

            println! {"***** Responses *****"}
            for resp in storage
                .get_challenge_responses(&shared_challenge.lock().unwrap())
                .unwrap()
                .iter()
            {
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
