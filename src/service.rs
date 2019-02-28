//! # Service
//!
//! Service chain interface and implementations

use bitcoin::util::hash::Sha256dHash;
use ocean_rpc::Client;
use request::{Bid, Request};

/// Service trait defining functionality for interfacing with service chain
pub trait Service {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<Request>>, &str>;

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &Sha256dHash) -> Result<Option<Request>, &str>;

    /// Try get active request bids, by genesis hash, from service chain
    fn get_request_bids(&self, hash: &Sha256dHash) -> Result<Option<Vec<Bid>>, &str>;
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
    fn get_requests(&self) -> Result<Option<Vec<Request>>, &str> {
        Ok(None)
    }

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, _hash: &Sha256dHash) -> Result<Option<Request>, &str> {
        Ok(None)
    }

    /// Try get active request bids, by genesis hash, from service chain
    fn get_request_bids(&self, _hash: &Sha256dHash) -> Result<Option<Vec<Bid>>, &str> {
        Ok(None)
    }
}
