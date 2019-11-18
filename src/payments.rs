//! Payments
//!
//! TODO: Add description

use std::str::FromStr;
use std::sync::mpsc::{Receiver, RecvError};
use std::sync::Arc;
use std::thread;

use bitcoin::hashes::sha256d;
use bitcoin::Amount;
use ocean::{Address, AddressParams};
use ocean_rpc::RpcApi;

use crate::config::ClientChainConfig;
use crate::error::{CError, Error, Result};
use crate::interfaces::{request::Request, storage::Storage};
use crate::util::ocean::OceanClient;

/// Get addr params from chain name
pub fn get_chain_addr_params(chain: &String) -> AddressParams {
    match chain.to_lowercase().as_ref() {
        "ocean_main" => return AddressParams::OCEAN,
        "gold_main" => return AddressParams::GOLD,
        _ => AddressParams::ELEMENTS,
    }
}

/// TODO: add comments
fn calculate_fees(request: &Request, client: &OceanClient) -> Result<Amount> {
    let mut fee_sum = Amount::ZERO;
    for i in request.start_blockheight_clientchain..request.end_blockheight_clientchain {
        let block = client.get_block_info(&client.get_block_hash(i.into())?)?;
        // using raw rpc to get asset label
        // check also coinbase destination ?
        // check is coinbase
        // check is correct label
        // check ownership
        let tx = client.get_raw_transaction_verbose(&block.tx[0], None)?;
        for txout in tx.vout {
            // do label check :)
            fee_sum += txout.value;
        }
    }
    Ok(fee_sum)
}

/// TODO: add comments
fn do_request_payment(
    clientchain_config: &ClientChainConfig,
    request: &Request,
    client: &OceanClient,
    storage: &Arc<dyn Storage>,
) -> Result<()> {
    let bids = storage.get_bids(request.txid)?;
    let resp = storage.get_response(request.txid)?;
    let amount = calculate_fees(request, client)?;
    info!("amount: {}", amount);
    Ok(())

    // calculate_fees()
    // for bids:
    //     lookup in resps
    //     get address
    //     pay
    //     store
    //     update Bid
    // update Request
}

/// TODO: add comments
fn do_request_payments(
    clientchain_config: ClientChainConfig,
    client: OceanClient,
    storage: Arc<dyn Storage>,
    req_recv: Receiver<sha256d::Hash>,
) -> Result<()> {
    // First pay out any past requests that have not been fully paid yet
    // TODO: only get incomplete only from storage
    let incomplete_requests = storage.get_requests()?;
    for req in incomplete_requests {
        let _ = do_request_payment(&clientchain_config, &req, &client, &storage)?;
    }

    // Wait for new requests
    loop {
        match req_recv.recv() {
            Ok(resp) => {
                let req = storage.get_request(resp)?.unwrap();
                let _ = do_request_payment(&clientchain_config, &req, &client, &storage)?;
            }
            Err(RecvError) => {
                return Err(Error::from(CError::ReceiverDisconnected));
            }
        }
    }
}

/// TODO: add comments
pub fn run_payments(
    clientchain_config: ClientChainConfig,
    storage: Arc<dyn Storage + Send + Sync>,
    req_recv: Receiver<sha256d::Hash>,
) -> Result<thread::JoinHandle<()>> {
    let client = OceanClient::new(
        clientchain_config.host.clone(),
        Some(clientchain_config.user.clone()),
        Some(clientchain_config.pass.clone()),
    )?;

    if let Some(addr) = &clientchain_config.payment_addr {
        let ocean_addr = Address::from_str(&addr)?;
        if *ocean_addr.params != get_chain_addr_params(&clientchain_config.chain) {
            warn!("payment addr and chain config addr param mismatch");
        } else if let Some(key) = &clientchain_config.payment_key {
            let addr_unspent = client.list_unspent(None, None, Some(&[ocean_addr]), None, None)?;
            if addr_unspent.len() == 0 {
                client.import_priv_key(key, None, Some(true))?;
            }
        } else {
            warn!("payment key missing");
        }
    } else {
        warn!("payment addr missing");
    }

    Ok(thread::spawn(move || {
        if let Err(err) = do_request_payments(clientchain_config, client, storage.clone(), req_recv) {
            error! {"payments error: {}", err}
        }
    }))
}
