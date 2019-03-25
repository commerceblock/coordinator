//! # Service
//!
//! Service chain interface and implementations

use std::str::FromStr;

use bitcoin_hashes::{hex::FromHex, sha256d};
use secp256k1::key::PublicKey;

use crate::error::{Error, Result};
use crate::ocean::RpcClient;
use crate::request::{Bid, BidSet, Request};

/// Service trait defining functionality for interfacing with service chain
pub trait Service {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<Request>>>;

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &sha256d::Hash) -> Result<Option<Request>>;

    /// Try get active request bids, by genesis hash, from service chain
    fn get_request_bids(&self, hash: &sha256d::Hash) -> Result<Option<BidSet>>;
}

/// Rpc implementation of Service using an underlying ocean rpc connection
pub struct RpcService {
    /// Rpc client instance
    client: RpcClient,
}

impl RpcService {
    /// Create an RpcService with underlying rpc client connectivity
    pub fn new() -> Result<Self> {
        Ok(RpcService {
            client: RpcClient::new(String::new(), Some(<String>::new()), Some(<String>::new()))?,
        })
    }
}

//
// TODO: implement Service trait for RpcService
//

/// Mock implementation of Service using some mock logic for testing
pub struct MockService {
    /// Flag that when set returns error on all inherited methods that return
    /// Result
    pub return_err: bool,
    /// Flag that when set returns None on all inherited methods that return
    /// Option
    pub return_none: bool,
}

impl MockService {
    /// Create a MockService with all flags turned off by default
    pub fn new() -> Self {
        MockService {
            return_err: false,
            return_none: false,
        }
    }
}

impl Service for MockService {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<Request>>> {
        Ok(None)
    }

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &sha256d::Hash) -> Result<Option<Request>> {
        if self.return_none {
            return Ok(None);
        }
        if self.return_err {
            return Err(Error::Coordinator("get_request failed".to_owned()));
        }
        let dummy_req = Request {
            start_blockheight: 2,
            end_blockheight: 5,
            genesis_blockhash: *hash,
            fee_percentage: 5,
            num_tickets: 10,
        };
        Ok(Some(dummy_req))
    }

    /// Try get active request bids, by genesis hash, from service chain
    fn get_request_bids(&self, _hash: &sha256d::Hash) -> Result<Option<BidSet>> {
        if self.return_none {
            return Ok(None);
        }
        if self.return_err {
            return Err(Error::Coordinator("get_request_bids failed".to_owned()));
        }
        let dummy_bid = Bid {
            txid: sha256d::Hash::from_hex("1234567890000000000000000000000000000000000000000000000000000000").unwrap(),
            // pubkey corresponding to SecretKey::from_slice(&[0xaa; 32])
            pubkey: PublicKey::from_str("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3").unwrap(),
        };
        let mut bid_set = BidSet::new();
        let _ = bid_set.insert(dummy_bid);
        Ok(Some(bid_set))
    }
}
