//! # Service
//!
//! Service chain interface and implementations

use bitcoin_hashes::sha256d;
use ocean_rpc::RpcApi;

use crate::config::ServiceConfig;
use crate::error::Result;
use crate::request::{Bid, BidSet, Request};
use crate::util::ocean::OceanClient;

/// Service trait defining functionality for interfacing with service chain
pub trait Service {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<Request>>>;
    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &sha256d::Hash) -> Result<Option<Request>>;
    /// Try get active request bids, by transaction hash, from service chain
    fn get_request_bids(&self, hash: &sha256d::Hash) -> Result<Option<BidSet>>;
    /// Get service chain blockheight
    fn get_blockheight(&self) -> Result<u64>;
}

/// Rpc implementation of Service using an underlying ocean rpc connection
pub struct RpcService {
    /// Rpc client instance
    client: OceanClient,
}

impl RpcService {
    /// Create an RpcService with underlying rpc client connectivity
    pub fn new(service_config: &ServiceConfig) -> Result<Self> {
        let client = OceanClient::new(
            service_config.host.clone(),
            Some(service_config.user.clone()),
            Some(service_config.pass.clone()),
        )?;

        let _ = client.get_block_count()?; // check connectivity

        Ok(RpcService { client })
    }
}

impl Service for RpcService {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<Request>>> {
        let resp = self.client.get_requests(None)?;
        let mut requests = vec![];
        for res in resp {
            requests.push(Request::from_json(&res));
        }
        Ok(Some(requests))
    }

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &sha256d::Hash) -> Result<Option<Request>> {
        let resp = self.client.get_requests(Some(hash))?;
        if resp.len() > 0 {
            return Ok(Some(Request::from_json(&resp[0])));
        }
        Ok(None)
    }

    /// Try get active request bids, by transaction hash, from service chain
    fn get_request_bids(&self, hash: &sha256d::Hash) -> Result<Option<BidSet>> {
        let resp = self.client.get_request_bids(hash)?;
        match resp {
            Some(res) => {
                let mut bids = BidSet::new();
                for bid in res.bids {
                    let _ = bids.insert(Bid::from_json(&bid));
                }
                return Ok(Some(bids));
            }
            None => Ok(None),
        }
    }

    /// Get service chain blockheight
    fn get_blockheight(&self) -> Result<u64> {
        Ok(self.client.get_block_count()?)
    }
}
