//! Coordinator
//!
//! Coordinator entry point for spawning all components

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::{thread, time};

use bitcoin::hashes::{hex::FromHex, sha256d};

use crate::challenger::{ChallengeResponse, ChallengeState};
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

    let api_handler = ::api::run_api_server(&config.api, storage.clone());
    let (req_send, req_recv): (Sender<sha256d::Hash>, Receiver<sha256d::Hash>) = channel();
    let mut payments_handler = ::payments::run_payments(config.clientchain.clone(), storage.clone(), req_recv)?;

    // create a challenge state mutex to share between challenger and listener.
    // initially None
    let shared_challenge = Arc::new(RwLock::new(None));
    // and a channel for sending responses from listener to challenger
    let (verify_tx, verify_rx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();
    // start listener along with a oneshot channel to send shutdown message
    let listener_handle = ::listener::run_listener(&config.listener_host, shared_challenge.clone(), verify_tx);

    // This loop runs continuously fetching and running challenge requests,
    // generating challenge responses and fails on any errors that occur
    loop {
        match run_request(
            &config,
            &service,
            &clientchain,
            storage.clone(),
            shared_challenge.clone(),
            &verify_rx,
            genesis_hash,
        ) {
            Ok(res) => {
                if let Some(request_id) = res {
                    // if challenge request succeeds print responses
                    req_send.send(request_id).unwrap();
                    info! {"***** Response *****"}
                    let resp = storage.get_response(request_id)?.unwrap();
                    info! {"{}", serde_json::to_string_pretty(&resp).unwrap()};
                }
                // Reset challenge state to None.
                *shared_challenge.write().unwrap() = None;

                info! {"Sleeping for {} sec...", config.block_time}
                thread::sleep(time::Duration::from_secs(config.block_time))
            }
            Err(err) => {
                api_handler.close(); // try closing the api server
                payments_handler.stop(); // try closing the payments service
                listener_handle.stop(); // try stop listener service
                return Err(err);
            }
        }
        if payments_handler.got_err() {
            break;
        }
    }
    api_handler.close(); // try closing the api server
    listener_handle.stop(); // try stop listener service
    Ok(())
}

/// Run request method attemps to fetch a challenge request and run it
/// This involves storing the Request and winning bids, issuing challenges
/// on the client chain and listening for responses on these challenges
pub fn run_request<T: Service, K: ClientChain, D: Storage>(
    config: &Config,
    service: &T,
    clientchain: &K,
    storage: Arc<D>,
    shared_challenge: Arc<RwLock<Option<ChallengeState>>>,
    verify_rx: &Receiver<ChallengeResponse>,
    genesis_hash: sha256d::Hash,
) -> Result<Option<sha256d::Hash>> {
    match ::challenger::fetch_next(service, &genesis_hash)? {
        Some(mut challenge) => {
            // First attempt to store the challenge state information
            // on requests and winning bids and exit if it fails.
            // If already set update challenge state with correct version from storage
            ::challenger::update_challenge_request_state(
                clientchain,
                service,
                storage.clone(),
                &mut challenge,
                config.block_time,
                config.clientchain.block_time,
            )?;

            // modify challenge state for the new challenge request
            *shared_challenge.write().unwrap() = Some(challenge);

            // run challenge request storing expected responses
            match ::challenger::run_challenge_request(
                service,
                clientchain,
                shared_challenge.clone(),
                &verify_rx,
                storage.clone(),
                time::Duration::from_secs(5 * config.block_time),
                time::Duration::from_secs(config.challenge_duration),
                config.challenge_frequency,
                time::Duration::from_secs(config.block_time / 2),
            ) {
                Ok(()) => {
                    // update end clientchain height with final height
                    let mut shared_ch_lock = shared_challenge.write().unwrap();
                    let ch_final = shared_ch_lock.as_mut().unwrap();
                    ch_final.request.end_blockheight_clientchain = clientchain.get_blockheight()?;
                    info!(
                        "Request client chain end height updated to {}",
                        ch_final.request.end_blockheight_clientchain
                    );
                    storage.update_request(&ch_final.request)?;
                    return Ok(Some(ch_final.request.txid));
                }
                Err(err) => Err(err),
            }
        }
        None => Ok(None),
    }
}
