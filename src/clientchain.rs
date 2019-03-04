//! # ClientChain
//!
//! Client chain interface and implementations

use bitcoin::util::hash::Sha256dHash;
use bitcoin_hashes::hex::ToHex;
use ocean_rpc::Client;

use crate::error::Result;

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
pub struct MockClientChain {}

impl ClientChain for MockClientChain {
    /// Get client chain blockheight
    fn get_blockheight(&self) -> Result<u64> {
        Ok(0)
    }

    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<Sha256dHash> {
        Ok(Sha256dHash::from_hex(&vec![0; 32].to_hex()).unwrap())
    }

    /// Verify challenge transaction has been included in the chain
    fn verify_challenge(&self, _txid: &Sha256dHash) -> Result<bool> {
        Ok(true)
    }
}
