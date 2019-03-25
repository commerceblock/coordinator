//! Coordinator
//!
//! Coordinator entry point for spawning all components

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin_hashes::{hex::FromHex, sha256d};
use futures::sync::oneshot;

use crate::challenger::ChallengeResponse;
use crate::clientchain::{ClientChain, RpcClientChain};
use crate::config::Config;
use crate::error::Result;
use crate::service::{MockService, Service};
use crate::storage::{MockStorage, Storage};

/// Run coordinator main method
pub fn run(config: Config) -> Result<()> {
    info!("Running coordinator!");

    let service = MockService::new();
    let clientchain = RpcClientChain::new(&config.clientchain)?;
    let storage = MockStorage::new();
    let genesis_hash = sha256d::Hash::from_hex(&config.clientchain.genesis_hash)?;

    run_inner(&config, &service, &clientchain, &storage, genesis_hash)
}

/// Inner run coordinator method with interfaces
pub fn run_inner<T: Service, K: ClientChain, D: Storage>(
    config: &Config,
    service: &T,
    clientchain: &K,
    storage: &D,
    genesis_hash: sha256d::Hash,
) -> Result<()> {
    // This loop runs until a challenge request is successfully
    // finished or an error occurs
    loop {
        match ::challenger::fetch_next(service, clientchain, &genesis_hash) {
            Ok(next) => {
                if let Some(challenge) = next {
                    // first attempt to store the challenge state information
                    // on requests and winning bids and exit if that fails
                    storage.save_challenge_state(&challenge)?;

                    // create a challenge state mutex to share between challenger and listener
                    let mut shared_challenge = Arc::new(Mutex::new(challenge));
                    // and a channel for sending responses from listener to challenger
                    let (verify_tx, verify_rx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

                    // start listener along with a oneshot channel to send shutdown message
                    let (thread_tx, thread_rx) = oneshot::channel();
                    let verify_handle =
                        ::listener::run_listener(&config.listener_host, shared_challenge.clone(), verify_tx, thread_rx);

                    // run challenge request storing expected responses
                    ::challenger::run_challenge_request(
                        clientchain,
                        shared_challenge.clone(),
                        &verify_rx,
                        storage,
                        time::Duration::from_secs(config.verify_duration),
                        time::Duration::from_secs(config.challenge_duration),
                    )?;

                    // if challenge request succeeds print responses
                    // TODO: how to propagate responses to fee payer
                    println! {"***** Responses *****"}
                    for resp in storage
                        .get_challenge_responses(&shared_challenge.lock().unwrap())
                        .unwrap()
                        .iter()
                    {
                        println! {"{:?}", resp}
                    }

                    // stop listener service
                    thread_tx.send(()).expect("thread_tx send failed");
                    verify_handle.join().expect("verify_handle join failed");
                    break;
                }
            }
            Err(e) => warn!("challenger fetch error: {}", e),
        }
        info! {"Sleeping for 1 sec..."}
        thread::sleep(time::Duration::from_secs(1))
    }
    Ok(())
}
