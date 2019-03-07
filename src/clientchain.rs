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
    /// Mock client chain blockheight - incremented by default on
    /// get_blockheight
    pub height: u64,
}

impl MockClientChain {
    /// Create a MockClientChain with all flags turned off by default
    pub fn new() -> Self {
        MockClientChain {
            return_err: false,
            return_false: false,
            height: 0,
        }
    }
}

impl MockClientChain {
    fn static_height(incr: bool) -> u64 {
        static HEIGHT: AtomicUsize = ATOMIC_USIZE_INIT;
        if incr {
            HEIGHT.fetch_add(1, Ordering::SeqCst) as u64
        } else {
            HEIGHT.load(Ordering::SeqCst) as u64
        }
    }
}

impl ClientChain for MockClientChain {
    /// Get client chain blockheight
    fn get_blockheight(&self) -> Result<u64> {
        if self.return_err {
            return Err(CError::Coordinator("get_blockheight failed"));
        }

        // when height is set return that (unit-testing ease)
        if self.height > 0 {
            return Ok(self.height);
        }

        // If height not set then use static counter
        Ok(MockClientChain::static_height(true))
    }

    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<Sha256dHash> {
        if self.return_err {
            return Err(CError::Coordinator("send_challenge failed"));
        }

        // if height is set use it for hash generation
        if self.height > 0 {
            return Ok(
                Sha256dHash::from_hex(&vec![(self.height % 16) as u8; 32].to_hex()).unwrap(),
            );
        }
        // If height is not set use static counter to generate random hash
        Ok(Sha256dHash::from_hex(
            &vec![(MockClientChain::static_height(false) % 16) as u8; 32].to_hex(),
        )
        .unwrap())
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
