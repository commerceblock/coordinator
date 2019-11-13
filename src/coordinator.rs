//! Coordinator
//!
//! Coordinator entry point for spawning all components

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin_hashes::{hex::FromHex, sha256d};
use futures::sync::oneshot;

use crate::challenger::ChallengeResponse;
use crate::config::Config;
use crate::error::Result;
use crate::interfaces::clientchain::{ClientChain, RpcClientChain};
use crate::interfaces::service::{RpcService, Service};
use crate::interfaces::storage::{MongoStorage, Storage};

/// Run coordinator main method
pub fn run(config: Config) -> Result<()> {
    info!("Running coordinator!");

    let service = RpcService::new(&config.service)?;
    let clientchain = RpcClientChain::new(&config.clientchain)?;
    let storage = Arc::new(MongoStorage::new(config.storage.clone())?);
    let genesis_hash = sha256d::Hash::from_hex(&config.clientchain.genesis_hash)?;

    let _ = ::api::run_api_server(&config.api, storage.clone());

    // This loop runs continuously fetching and running challenge requests,
    // generating challenge responses and fails on any errors that occur
    loop {
        if let Some(request_id) = run_request(&config, &service, &clientchain, storage.clone(), genesis_hash)? {
            // if challenge request succeeds print responses
            info! {"***** Response *****"}
            let resp = storage.get_response(request_id)?.unwrap();
            info! {"{}", serde_json::to_string_pretty(&resp).unwrap()};
        }
        info! {"Sleeping for {} sec...", config.block_time}
        thread::sleep(time::Duration::from_secs(config.block_time))
    }
}

/// Run request method attemps to fetch a challenge request and run it
/// This involves storing the Request and winning bids, issuing challenges
/// on the client chain and listening for responses on these challenges
pub fn run_request<T: Service, K: ClientChain, D: Storage>(
    config: &Config,
    service: &T,
    clientchain: &K,
    storage: Arc<D>,
    genesis_hash: sha256d::Hash,
) -> Result<Option<sha256d::Hash>> {
    match ::challenger::fetch_next(service, &genesis_hash)? {
        Some(mut challenge) => {
            // First attempt to store the challenge state information
            // on requests and winning bids and exit if it fails.
            // If already set update challenge state with correct version from storage
            ::challenger::update_challenge_request_state(config, clientchain, storage.clone(), &mut challenge)?;

            // create a challenge state mutex to share between challenger and listener
            let shared_challenge = Arc::new(Mutex::new(challenge));
            // and a channel for sending responses from listener to challenger
            let (verify_tx, verify_rx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

            // start listener along with a oneshot channel to send shutdown message
            let (thread_tx, thread_rx) = oneshot::channel();
            let verify_handle =
                ::listener::run_listener(&config.listener_host, shared_challenge.clone(), verify_tx, thread_rx);

            // run challenge request storing expected responses
            ::challenger::run_challenge_request(
                service,
                clientchain,
                shared_challenge.clone(),
                &verify_rx,
                storage.clone(),
                time::Duration::from_secs(5 * config.block_time),
                time::Duration::from_secs(config.challenge_duration),
                config.challenge_frequency,
                time::Duration::from_secs(config.block_time / 2),
            )?;

            // stop listener service
            thread_tx.send(()).expect("thread_tx send failed");
            verify_handle.join().expect("verify_handle join failed");

            return Ok(Some(shared_challenge.lock().unwrap().request.txid));
        }
        None => Ok(None),
    }
}
