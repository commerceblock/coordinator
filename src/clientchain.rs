//! # ClientChain
//!
//! Client chain interface and implementations

use std::cell::RefCell;

use bitcoin::util::hash::Sha256dHash;
use ocean_rpc::{Client, RpcApi};

use crate::error::{CError, Result};

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
pub struct RpcClientChain {
    client: Client,
}

impl RpcClientChain {
    /// Create an RpcClientChain with underlying rpc client connectivity
    pub fn new(url: String, user: Option<String>, pass: Option<String>) -> Result<Self> {
        let client = Client::new(url, user, pass);
        let unspent = client.list_unspent(None, None, None, None, None)?;

        // Check challenge asset hash is in the wallet
        // TODO: asset == challenge_asset from config
        // TODO: address == challenge_address from config
        // TODO: replace "CBT" with challenge label when added
        let mut found = false;
        for tx in unspent.iter() {
            if tx.assetlabel == Some("CBT".into()) {
                found = true;
                break;
            }
        }

        // TODO: custom error for clientchain
        if !found {
            return Err(CError::Coordinator("no challenge asset balance found"));
        }

        Ok(RpcClientChain { client })
    }
}

impl ClientChain for RpcClientChain {
    /// Get client chain blockheight
    fn get_blockheight(&self) -> Result<u64> {
        match self.client.get_block_count() {
            Ok(res) => Ok(res),
            Err(e) => Err(CError::OceanRpc(e)),
        }
    }

    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<Sha256dHash> {
        Ok(Sha256dHash::from(&[0u8; 32] as &[u8]))
    }

    /// Verify challenge transaction has been included in the chain
    fn verify_challenge(&self, txid: &Sha256dHash) -> Result<bool> {
        let tx = self.client.get_raw_transaction_verbose(txid, None)?;

        if let (Some(_hash), Some(n_conf)) = (tx.blockhash, tx.confirmations) {
            if n_conf > 0 {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

//
// TODO: implement ClientChain trait for RpcClientChain
//

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
