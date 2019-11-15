//! Payments
//!
//! TODO: Add description

use std::sync::mpsc::{Receiver, RecvError};
use std::sync::Arc;
use std::thread;

use bitcoin::hashes::sha256d;
use bitcoin::Amount;
use ocean_rpc::RpcApi;

use crate::config::ClientChainConfig;
use crate::error::{CError, Error, Result};
use crate::interfaces::{request::Request, storage::Storage};
use crate::util::ocean::OceanClient;

///
fn calculate_fees(request: &Request, client: &OceanClient) -> Result<Amount> {
    let mut fee_sum = Amount::ZERO;
    for i in request.start_blockheight_clientchain..request.end_blockheight_clientchain {
        let block = client.get_block_info(&client.get_block_hash(i.into())?)?;
        let tx = client.get_raw_transaction_verbose(&block.tx[0], None)?;
        for txout in tx.vout {
            // do label check :)
            fee_sum += txout.value;
        }
    }
    Ok(fee_sum)
}

///
fn do_request_payment(request: &Request, client: &OceanClient, storage: &Arc<dyn Storage>) -> Result<()> {
    let bids = storage.get_bids(request.txid)?;
    let resp = storage.get_response(request.txid)?;

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

///
fn do_request_payments(
    client: OceanClient,
    storage: Arc<dyn Storage>,
    req_recv: Receiver<sha256d::Hash>,
) -> Result<()> {
    // get payment mode from config

    // get addr prefix from config

    // First pay out any past requests that have not been fully paid yet
    let incomplete_requests = storage.get_requests()?; //paid = False)
    for req in incomplete_requests {
        let _ = do_request_payment(&req, &client, &storage)?;
    }

    // Wait for new requests
    loop {
        match req_recv.recv() {
            Ok(resp) => {
                let req = storage.get_request(resp)?.unwrap();
                let _ = do_request_payment(&req, &client, &storage)?;
            }
            Err(RecvError) => {
                return Err(Error::from(CError::ReceiverDisconnected));
            }
        }
    }
}

///
pub fn run_payments(
    clientchain_config: &ClientChainConfig,
    storage: Arc<dyn Storage + Send + Sync>,
    req_recv: Receiver<sha256d::Hash>,
) -> Result<thread::JoinHandle<()>> {
    let client = OceanClient::new(
        clientchain_config.host.clone(),
        Some(clientchain_config.user.clone()),
        Some(clientchain_config.pass.clone()),
    )?;

    Ok(thread::spawn(move || {
        if let Err(err) = do_request_payments(client, storage.clone(), req_recv) {
            error! {"payments error: {}", err}
        }
    }))
}
