//! Payments
//!
//! TODO: Add description

use std::str::FromStr;
use std::sync::mpsc::{Receiver, RecvError};
use std::sync::Arc;
use std::thread;

use bitcoin::{hashes::sha256d, Amount, PublicKey};
use ocean::{Address, AddressParams};
use ocean_rpc::{json::SendAnyToAddressResult, RpcApi};

use crate::config::ClientChainConfig;
use crate::error::{CError, Error, Result};
use crate::interfaces::{
    bid::{Bid, BidPayment},
    request::Request,
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

/// Function that calculates all the fees accumulated in the duration of a
/// service request in the clientchain
fn calculate_fees(request: &Request, client: &OceanClient) -> Result<Amount> {
    let mut fee_sum = Amount::ZERO;
    for i in request.start_blockheight_clientchain..=request.end_blockheight_clientchain {
        let block = client.get_block_info(&client.get_block_hash(i.into())?)?;
        let tx = client.get_raw_transaction_verbose(&block.tx[0], None)?; // coinbase tx
        assert!(tx.is_coinbase() == true);
        for txout in tx.vout {
            match txout.assetlabel {
                Some(label) => {
                    // any other label is a policy asset
                    if label == "CBT" {
                        fee_sum += txout.value;
                    }
                }
                None => fee_sum += txout.value,
            }
        }
    }
    Ok(fee_sum)
}

/// Function that calculates the fee amount to be received per bid given total
/// fees, fee percentage and bid number
fn calculate_bid_payment(fees_amount: &Amount, fee_percentage: u64, num_bids: u64) -> Result<Amount> {
    let total_amount = *fees_amount * fee_percentage / 100;
    Ok(total_amount / num_bids) // amount per bid
}

/// Payment Struct holding data and logic required to pay bids at the end of the
/// service request
pub struct Payments {
    /// Thread safe storage instance
    pub storage: Arc<dyn Storage + Send + Sync>,

    /// Ocean rpc connectivity to client chain
    pub client: OceanClient,
    /// Clientchain address params required for fee payments
    pub addr_params: &'static AddressParams,
    /// Payment asset with which fees rewards will be paid
    pub payment_asset: String,
    /// Flag that determines whether we do actual payments or just collect and
    /// store payment data
    pub do_payment: bool,
}

impl Payments {
    /// Method that does the actual payments to bid owners for the service
    /// request. Uses sendtoaddress if the asset label has been specified or
    /// sendanytoaddress if not. Errors don't kill the process but
    /// signal that payments have failed. Already paid bids are skipped.
    fn complete_bid_payments(&self, bids: &mut Vec<Bid>) -> Result<bool> {
        let use_sendany = self.payment_asset == "ANY";
        for bid in bids {
            if let Some(bid_payment) = bid.payment.as_mut() {
                if !bid_payment.txid.is_none() {
                    warn!(
                        "addr {} paid already (txid: {})",
                        &bid_payment.address,
                        bid_payment.txid.unwrap()
                    );
                    continue;
                }
                info!("payment to {} for {}", &bid_payment.address, bid_payment.amount);
                if use_sendany {
                    match self.client.send_any_to_address(
                        &bid_payment.address,
                        bid_payment.amount,
                        None,
                        None,
                        None,
                        Some(true),
                        None,
                    ) {
                        Ok(res) => {
                            match res {
                                SendAnyToAddressResult::Txid(txid) => {
                                    bid_payment.txid = Some(txid);
                                    info!("payment (ANY) txid {}", txid);
                                }
                                SendAnyToAddressResult::Txids(txids) => {
                                    bid_payment.txid = Some(txids[0]); // TODO: fix this to store all
                                    bid_payment.extra_txids = Some(txids[1..].to_vec());
                                    info!("payment (ANY) txids {:?}", txids);
                                }
                            }
                        }
                        Err(err) => {
                            warn!("bid payment (send_any_to_address) failed: {}", err);
                            return Ok(false);
                        }
                    }
                } else {
                    match self.client.send_to_address(
                        &bid_payment.address,
                        bid_payment.amount,
                        None,
                        None,
                        Some(false),
                        Some(&self.payment_asset),
                    ) {
                        Ok(txid) => {
                            bid_payment.txid = Some(txid);
                            info!("payment ({}) txid {}", &self.payment_asset, txid);
                        }
                        Err(err) => {
                            warn!("bid payment (send_to_address) failed: {}", err);
                            return Ok(false);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    /// Process bid payments method handles calculating the payment to be
    /// received per bid and on which address, and updates the corresponding
    /// payment info in Storage
    fn process_bid_payments(&self, bids: &mut Vec<Bid>, bid_payment: &Amount, response: &Response) -> Result<()> {
        for bid in bids {
            if let Some(bid_resp) = response.bid_responses.get(&bid.txid) {
                // correct bid payment by calculating the performance
                // base on successful responses / total responses
                let bid_payment_corrected = *bid_payment * (*bid_resp).into() / response.num_challenges.into();
                let bid_pay_to_addr = Address::p2pkh(
                    &PublicKey {
                        key: bid.pubkey,
                        compressed: true,
                    },
                    None,
                    self.addr_params,
                );

                bid.payment = Some(BidPayment {
                    amount: bid_payment_corrected,
                    address: bid_pay_to_addr,
                    txid: None,
                    extra_txids: None,
                });
            }
        }
        Ok(())
    }

    /// Method that handles payments for a single request, fetching bid
    /// information, calculating fees, updating payment information and doing
    /// payments. Requests are marked as payment complete if payments are done
    /// successfully or if the coordinator does not handle payments
    fn do_request_payment(&self, request: &mut Request) -> Result<()> {
        // skip requests that have not finished
        if request.end_blockheight_clientchain == 0
            || (self.client.get_block_count()? as u32) < request.end_blockheight_clientchain
        {
            warn! {"Skipping unfinished request: {}", request.txid};
            return Ok(());
        }

        // fetch bids, responses, update payment info and do payments
        let mut bids = self.storage.get_bids(request.txid)?;
        let mut payment_complete = true;
        if bids.len() > 0 {
            if let Some(resp) = self.storage.get_response(request.txid)? {
                let fees_amount = calculate_fees(request, &self.client)?;
                info! {"total service request fees: {}", fees_amount};
                let bid_payment_amount =
                    calculate_bid_payment(&fees_amount, request.fee_percentage.into(), bids.len() as u64)?;
                info! {"num bids: {} fee per bid: {} ({}%)", bids.len(), bid_payment_amount, request.fee_percentage};

                self.process_bid_payments(&mut bids, &bid_payment_amount, &resp)?;
                if self.do_payment {
                    payment_complete = self.complete_bid_payments(&mut bids)?
                }

                // update bids with payment information
                for bid in bids {
                    self.storage.update_bid(request.txid, &bid)?;
                }
            }
        }

        // update request with payment complete
        request.is_payment_complete = payment_complete;
        self.storage.update_request(request)?;
        Ok(())
    }

    /// Main Request payments method; first checks for any incomplete requests
    /// and then listens for new requests on the receiver channel
    fn do_request_payments(&self, req_recv: Receiver<sha256d::Hash>) -> Result<()> {
        // Look for incomplete requests
        let incomplete_requests = self.storage.get_requests(Some(false), None, None)?;
        for mut req in incomplete_requests {
            info! {"Found incomplete request: {} ", req.txid};
            let _ = self.do_request_payment(&mut req)?;
        }

        // Wait for new requests
        loop {
            match req_recv.recv() {
                Ok(resp) => {
                    let mut req = self.storage.get_request(resp)?.unwrap();
                    info! {"New request: {}", req.txid};
                    let _ = self.do_request_payment(&mut req)?;
                }
                Err(RecvError) => {
                    return Err(Error::from(CError::ReceiverDisconnected));
                }
            }
        }
    }

    /// Return new Payments instance that requires clientchain config for
    /// various payment info and rpc calls to calculate payment fees and do the
    /// payments as well as a thread-safe reference to a Storage instance for
    /// getting request information and updating payment details
    pub fn new(config: ClientChainConfig, storage: Arc<dyn Storage + Send + Sync>) -> Result<Payments> {
        let client = OceanClient::new(
            config.host.clone(),
            Some(config.user.clone()),
            Some(config.pass.clone()),
        )?;

        // Check if payment addr/key are set and import the key for payment funds
        let addr_params = get_chain_addr_params(&config.chain);
        let mut do_payment = false;
        if let Some(addr) = &config.payment_addr {
            let ocean_addr = Address::from_str(&addr)?;
            if *ocean_addr.params != *addr_params {
                warn!("payment addr and chain config addr param mismatch");
            } else {
                if client.list_unspent(None, None, Some(&[ocean_addr]), None, None)?.len() == 0 {
                    if let Some(key) = &config.payment_key {
                        client.import_priv_key(key, None, Some(true))?;
                    } else {
                        warn!("payment key missing");
                    }
                }
                // set payment flag if payment addr is set or key is imported
                do_payment = true;
            }
        } else {
            warn!("payment addr missing");
        }

        Ok(Payments {
            storage,
            client,
            addr_params,
            payment_asset: config.payment_asset,
            do_payment,
        })
    }
}

/// Run payments daemon in a separate thread with a Payments instance receiving
/// information on finished requests via a Receiver channel
pub fn run_payments(
    clientchain_config: ClientChainConfig,
    storage: Arc<dyn Storage + Send + Sync>,
    req_recv: Receiver<sha256d::Hash>,
) -> Result<thread::JoinHandle<()>> {
    let payments = Payments::new(clientchain_config, storage)?;
    Ok(thread::spawn(move || {
        if let Err(err) = payments.do_request_payments(req_recv) {
            error! {"payments error: {}", err};
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::util::testing::setup_logger;

    #[test]
    fn calculate_bid_payment_test() {
        setup_logger();
        assert_eq!(
            1.125,
            calculate_bid_payment(&Amount::from_btc(6.0).unwrap(), 75, 4)
                .unwrap()
                .as_btc()
        );
        assert_eq!(
            0.24,
            calculate_bid_payment(&Amount::from_btc(4.0).unwrap(), 30, 5)
                .unwrap()
                .as_btc()
        );
        assert_eq!(
            1.0,
            calculate_bid_payment(&Amount::from_btc(100.0).unwrap(), 1, 1)
                .unwrap()
                .as_btc()
        );
        assert_eq!(
            0.0,
            calculate_bid_payment(&Amount::from_btc(0.0).unwrap(), 1, 1)
                .unwrap()
                .as_btc()
        );
        assert_eq!(
            0.0,
            calculate_bid_payment(&Amount::from_btc(4.0).unwrap(), 0, 5)
                .unwrap()
                .as_btc()
        );
    }

    #[test]
    fn get_chain_addr_params_test() {
        setup_logger();
        assert_eq!(
            &AddressParams::OCEAN,
            get_chain_addr_params(&String::from("ocean_main"))
        );
        assert_eq!(&AddressParams::GOLD, get_chain_addr_params(&String::from("gold_main")));
        assert_eq!(
            &AddressParams::ELEMENTS,
            get_chain_addr_params(&String::from("supersilverhazechain"))
        );
    }
}
