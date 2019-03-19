//! # ClientChain
//!
//! Client chain interface and implementations

use std::cell::RefCell;
use std::collections::HashMap;

use bitcoin::consensus::encode::serialize;
use bitcoin::util::hash::Sha256dHash;
use ocean_rpc::{json, Client, RpcApi};

use crate::error::{CError, Result};

/// Method that returns the first unspent for a specified asset label
/// or an error if the client wallet does not have any unspent/funds
fn get_first_unspent(client: &Client, asset: &str) -> Result<json::ListUnspentResult> {
    // Check challenge asset hash is in the wallet
    // TODO maybe: address == challenge_address
    let unspent = client.list_unspent(None, None, None, None, None)?;
    for tx in unspent.iter() {
        if tx.assetlabel == Some(asset.into()) {
            return Ok(tx.clone());
        }
    }

    // TODO: custom error for clientchain
    Err(CError::Coordinator("no challenge asset balance found"))
}

/// ClientChain trait defining desired functionality for interfacing
/// with the client chain when coordinating the guardnode service
pub trait ClientChain {
    /// Get client chain blockheight
    fn get_blockheight(&self) -> Result<u64>;
    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<Sha256dHash>;
    /// Verify challenge transaction has been included in the chain
    fn verify_challenge(&self, txid: &Sha256dHash) -> Result<bool>;
}

/// Rpc implementation of Service using an underlying ocean rpc connection
pub struct RpcClientChain<'a> {
    client: Client,
    asset: &'a str,
}

impl<'a> RpcClientChain<'a> {
    /// Create an RpcClientChain with underlying rpc client connectivity
    pub fn new(url: String, user: Option<String>, pass: Option<String>, asset: &'a str) -> Result<Self> {
        let client = Client::new(url, user, pass);

        // check we have funds for challenge asset
        let _ = get_first_unspent(&client, asset)?;

        Ok(RpcClientChain { client, asset })
    }
}

impl<'a> ClientChain for RpcClientChain<'a> {
    /// Get client chain blockheight
    fn get_blockheight(&self) -> Result<u64> {
        Ok(self.client.get_block_count()?)
    }

    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<Sha256dHash> {
        // get any unspent for the challenge asset
        let unspent = get_first_unspent(&self.client, self.asset)?;

        // construct the challenge transaction excluding fees
        // which are not required for policy transactions
        let utxos = vec![json::CreateRawTransactionInput {
            txid: unspent.txid,
            vout: unspent.vout,
            sequence: None,
        }];

        let mut outs = HashMap::new();
        let _ = outs.insert(
            unspent.address.clone(),
            (unspent.amount.into_inner() / 100000000) as f64,
        );

        let mut outs_assets = HashMap::new();
        let _ = outs_assets.insert(unspent.address.clone(), unspent.asset.to_string());

        let tx = self
            .client
            .create_raw_transaction(&utxos, Some(&outs), Some(&outs_assets), None)?;

        // sign the transaction and send via the client rpc
        let signed_tx = self
            .client
            .sign_raw_transaction(serialize(&tx).as_slice().into(), None, None, None)?;

        Ok(Sha256dHash::from_hex(
            &self.client.send_raw_transaction(&signed_tx.hex)?,
        )?)
    }

    /// Verify challenge transaction has been included in the chain
    fn verify_challenge(&self, txid: &Sha256dHash) -> Result<bool> {
        match self.client.get_raw_transaction_verbose(txid, None) {
            Ok(tx) => {
                // check for blockhash and number of confirmations
                if let (Some(_hash), Some(n_conf)) = (tx.blockhash, tx.confirmations) {
                    if n_conf > 0 {
                        return Ok(true);
                    }
                }
            }
            // no error throwing as issue might have been caused by
            // not successfuly sending the transaction and is not critical
            Err(e) => warn!("verify challenge error{}", e),
        }
        Ok(false)
    }
}

/// Mock implementation of ClientChain using some mock logic for testing
pub struct MockClientChain {
    /// Flag that when set returns error on all inherited methods that return
    /// Result
    pub return_err: bool,
    /// Flag that when set returns false on all inherited methods that return
    /// bool
    pub return_false: bool,
    /// Mock client chain blockheight - incremented by default on
    /// get_blockheight
    pub height: RefCell<u64>,
}

impl MockClientChain {
    /// Create a MockClientChain with all flags turned off by default
    pub fn new() -> Self {
        MockClientChain {
            return_err: false,
            return_false: false,
            height: RefCell::new(0),
        }
    }
}

impl ClientChain for MockClientChain {
    /// Get client chain blockheight
    fn get_blockheight(&self) -> Result<u64> {
        if self.return_err {
            return Err(CError::Coordinator("get_blockheight failed"));
        }

        let mut height = self.height.borrow_mut();
        *height += 1; // increment height for integration testing
        Ok(*height - 1) // return previous height
    }

    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<Sha256dHash> {
        if self.return_err {
            return Err(CError::Coordinator("send_challenge failed"));
        }

        // Use height to generate mock challenge hash
        Ok(Sha256dHash::from(&[(*self.height.borrow() % 16) as u8; 32] as &[u8]))
    }

    /// Verify challenge transaction has been included in the chain
    fn verify_challenge(&self, _txid: &Sha256dHash) -> Result<bool> {
        if self.return_err {
            return Err(CError::Coordinator("verify_challenge failed"));
        }
        if self.return_false {
            return Ok(false);
        }
        Ok(true)
    }
}
