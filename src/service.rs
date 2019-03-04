//! # Service
//!
//! Service chain interface and implementations

use bitcoin::util::hash::Sha256dHash;
use bitcoin_hashes::hex::FromHex;
use ocean_rpc::Client;
use secp256k1::key::PublicKey;

use crate::error::Result;
use crate::request::{Bid, Request};

/// Service trait defining functionality for interfacing with service chain
pub trait Service {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<Request>>>;

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &Sha256dHash) -> Result<Option<Request>>;

    /// Try get active request bids, by genesis hash, from service chain
    fn get_request_bids(&self, hash: &Sha256dHash) -> Result<Option<Vec<Bid>>>;
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

// TODO
// implement Service trait, once rpc calls for request/bid exist in ocean_rpc
// impl Service for RpcService {
//     /// Get all active requests, if any, from service chain
//     fn get_requests(&self) -> Result<Option<Vec<Request>>, &str> {
//         Ok(None)
//     }

//     /// Try get active request, by genesis hash, from service chain
//     fn get_request(&self, _hash: &Sha256dHash) -> Result<Option<Request>,
// &str> {         Ok(None)
//     }

//     /// Try get active request bids, by genesis hash, from service chain
//     fn get_request_bids(&self, _hash: &Sha256dHash) ->
// Result<Option<Vec<Bid>>, &str> {         Ok(None)
//     }
// }

/// Mock implementation of Service using some mock logic for testing
pub struct MockService {}

impl Service for MockService {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<Request>>> {
        Ok(None)
    }

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &Sha256dHash) -> Result<Option<Request>> {
        let dummy_req = Request {
            start_blockheight: 1,
            end_blockheight: 3,
            genesis_blockhash: *hash,
            fee_percentage: 5,
            num_tickets: 10,
        };

        Ok(Some(dummy_req))
    }

    /// Try get active request bids, by genesis hash, from service chain
    fn get_request_bids(&self, _hash: &Sha256dHash) -> Result<Option<Vec<Bid>>> {
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
        Ok(Some(vec![dummy_bid]))
    }
}
