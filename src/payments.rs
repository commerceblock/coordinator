//! Payments
//!
//! TODO: Add description

use std::str::FromStr;
use std::sync::mpsc::{Receiver, RecvError};
use std::sync::Arc;
use std::thread;

use bitcoin::hashes::sha256d;
use bitcoin::Amount;
use bitcoin::PublicKey;
use ocean::{Address, AddressParams};
use ocean_rpc::RpcApi;

use crate::config::ClientChainConfig;
use crate::error::{CError, Error, Result};
use crate::interfaces::{
    request::{BidSet, Request},
    response::Response,
    storage::Storage,
};
use crate::util::ocean::OceanClient;

/// Get addr params from chain name
pub fn get_chain_addr_params(chain: &String) -> &'static AddressParams {
    match chain.to_lowercase().as_ref() {
        "ocean_main" => return &AddressParams::OCEAN,
        "gold_main" => return &AddressParams::GOLD,
        _ => &AddressParams::ELEMENTS,
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

/// TODO
fn calculate_bid_payment(fee_amount: &Amount, fee_percentage: u64, num_bids: u64) -> Result<Amount> {
    info!("amount: {}", fee_amount);
    let gn_amount = *fee_amount * fee_percentage / 100;
    info!("gn_amount: {}", gn_amount);
    let gn_amount_per_gn = gn_amount / num_bids;
    info!("gn_amount_per_gn: {}", gn_amount_per_gn);
    Ok(gn_amount_per_gn)
}

/// TODO
fn pay_bids(_client: &OceanClient, _clientchain_config: &ClientChainConfig, _bids: &BidSet) -> Result<()> {
    // pay
    // set bid txid
    // update bid
    Ok(())
}

/// TODO
fn process_bids(
    bids: &BidSet,
    bids_amount: &Amount,
    response: &Response,
    addr_params: &'static AddressParams,
) -> Result<()> {
    for bid in bids {
        if let Some(bid_resp) = response.bid_responses.get(&bid.txid) {
            let gn_amount_corrected = *bids_amount * (*bid_resp).into() / response.num_challenges.into();
            let das_pub = PublicKey {
                key: bid.pubkey,
                compressed: true,
            };
            let gn_pay_to_addr = Address::p2pkh(&das_pub, None, addr_params);
            info!(
                "bid: {}\naddr: {}\ngn_amount_corrected: {}\n",
                bid.txid, gn_pay_to_addr, gn_amount_corrected
            );

            // set bid amount, addr
            // update bid
        }
    }
    Ok(())
}

/// TODO: add comments
fn do_request_payment(
    clientchain_config: &ClientChainConfig,
    request: &Request,
    client: &OceanClient,
    storage: &Arc<dyn Storage>,
) -> Result<()> {
    let bids = storage.get_bids(request.txid)?;
    if bids.len() > 0 {
        if let Some(resp) = storage.get_response(request.txid)? {
            let amount = calculate_fees(request, client)?;
            let bid_payment = calculate_bid_payment(&amount, request.fee_percentage.into(), bids.len() as u64)?;
            process_bids(
                &bids,
                &bid_payment,
                &resp,
                get_chain_addr_params(&clientchain_config.chain),
            )?;
            //pay_bids()?;
        }
    }

    // set request complete
    // update request

    Ok(())
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
        if *ocean_addr.params != *get_chain_addr_params(&clientchain_config.chain) {
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
