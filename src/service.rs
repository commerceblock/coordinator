//! # Service
//!
//! Service chain interface and implementations

use bitcoin::util::hash::Sha256dHash;
use bitcoin_hashes::hex::FromHex;
use ocean_rpc::Client;
use secp256k1::key::PublicKey;

use crate::error::{CError, Result};
use crate::request::{Bid, BidSet, Request};

/// Service trait defining functionality for interfacing with service chain
pub trait Service {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<Request>>>;

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &Sha256dHash) -> Result<Option<Request>>;

    /// Try get active request bids, by genesis hash, from service chain
    fn get_request_bids(&self, hash: &Sha256dHash) -> Result<Option<BidSet>>;
}

/// Rpc implementation of Service using an underlying ocean rpc connection
pub struct RpcService {
    client: Client,
}

impl RpcService {
    /// Create an RpcService with underlying rpc client connectivity
    pub fn new() -> Self {
        RpcService {
            client: Client::new(String::new(), Some(<String>::new()), Some(<String>::new())),
        }
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
    fn get_request(&self, hash: &Sha256dHash) -> Result<Option<Request>> {
        if self.return_none {
            return Ok(None);
        }
        if self.return_err {
            return Err(CError::Coordinator("get_request failed"));
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
    fn get_request_bids(&self, _hash: &Sha256dHash) -> Result<Option<BidSet>> {
        if self.return_none {
            return Ok(None);
        }
        if self.return_err {
            return Err(CError::Coordinator("get_request_bids failed"));
        }
        let dummy_bid = Bid {
            txid: Sha256dHash::from_hex(
                "1234567890000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            pubkey: PublicKey::from_slice(
                &Vec::<u8>::from_hex(
                    "03356190524d52d7e94e1bd43e8f23778e585a4fe1f275e65a06fa5ceedb67d2f3",
                )
                .unwrap(),
            )
            .unwrap(),
        };
        let mut bid_set = BidSet::new();
        let _ = bid_set.insert(dummy_bid);
        Ok(Some(bid_set))
    }
}
