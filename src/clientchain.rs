//! # ClientChain
//!
//! Client chain interface and implementations

use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

use bitcoin::util::hash::Sha256dHash;
use bitcoin_hashes::hex::ToHex;
use ocean_rpc::Client;

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
    pub fn new() -> Self {
        RpcClientChain {
            client: Client::new(String::new(), Some(<String>::new()), Some(<String>::new())),
        }
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
}

impl MockClientChain {
    /// Create an RpcClientChain with underlying rpc client connectivity
    pub fn new() -> Self {
        MockClientChain {
            return_err: false,
            return_false: false,
        }
    }
}

impl ClientChain for MockClientChain {
    /// Get client chain blockheight
    fn get_blockheight(&self) -> Result<u64> {
        if self.return_err {
            return Err(CError::Coordinator("get_blockheight failed"));
        }
        // Mock implementation increments a static counter
        // each time this method is called
        static HEIGHT: AtomicUsize = ATOMIC_USIZE_INIT;
        Ok(HEIGHT.fetch_add(1, Ordering::SeqCst) as u64)
    }

    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<Sha256dHash> {
        if self.return_err {
            return Err(CError::Coordinator("send_challenge failed"));
        }
        Ok(Sha256dHash::from_hex(&vec![0; 32].to_hex()).unwrap())
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
