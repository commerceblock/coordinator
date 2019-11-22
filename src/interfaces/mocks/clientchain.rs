//! Mock clientchain
//!
//! Mock clientchain implementation for testing

use bitcoin::hashes::{sha256d, Hash};
use std::cell::RefCell;

use crate::error::*;
use crate::interfaces::clientchain::ClientChain;

/// Mock implementation of ClientChain using some mock logic for testing
pub struct MockClientChain {
    /// Flag that when set returns error on all inherited methods that return
    /// Result
    pub return_err: bool,
    /// Flag that when set returns false on all inherited methods that return
    /// bool
    pub return_false: bool,
    /// Mock client chain blockheight
    pub height: RefCell<u32>,
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
    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<sha256d::Hash> {
        if self.return_err {
            return Err(Error::from(CError::Generic("send_challenge failed".to_owned())));
        }
        // Use height to generate mock challenge hash
        Ok(sha256d::Hash::from_slice(&[(*self.height.borrow() % 16) as u8; 32])?)
    }

    /// Verify challenge transaction has been included in the chain
    fn verify_challenge(&self, _txid: &sha256d::Hash) -> Result<bool> {
        if self.return_err {
            return Err(Error::from(CError::Generic("verify_challenge failed".to_owned())));
        }
        if self.return_false {
            return Ok(false);
        }
        Ok(true)
    }

    /// Get block count dummy
    fn get_blockheight(&self) -> Result<u32> {
        Ok(self.height.clone().into_inner())
    }
}
