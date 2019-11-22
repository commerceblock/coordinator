//! Mock service
//!
//! Mock service implementation for testing

use bitcoin::hashes::{hex::FromHex, sha256d, Hash};
use bitcoin::secp256k1::PublicKey;
use std::cell::RefCell;
use std::str::FromStr;

use crate::error::{CError, Error, Result};
use crate::interfaces::service::Service;
use crate::interfaces::{
    bid::{Bid, BidSet},
    request::Request as ServiceRequest,
};

/// Mock implementation of Service using some mock logic for testing
pub struct MockService {
    /// Flag that when set returns error on all inherited methods that return
    /// Result
    pub return_err: bool,
    /// Flag that when set returns None on all inherited methods that return
    /// Option
    pub return_none: bool,
    /// Current active request
    pub request: RefCell<ServiceRequest>,
    /// Mock service chain blockheight - incremented by default on
    /// get_blockheight
    pub height: RefCell<u64>,
}

impl MockService {
    /// Create a MockService with all flags turned off by default
    pub fn new() -> Self {
        let request = ServiceRequest {
            txid: sha256d::Hash::from_slice(&[0xff as u8; 32]).unwrap(),
            start_blockheight: 2,
            end_blockheight: 5,
            genesis_blockhash: sha256d::Hash::from_hex(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            fee_percentage: 5,
            num_tickets: 10,
            start_blockheight_clientchain: 0,
            end_blockheight_clientchain: 0,
            is_payment_complete: false,
        };

        MockService {
            return_err: false,
            return_none: false,
            request: RefCell::new(request),
            height: RefCell::new(0),
        }
    }
}

impl Service for MockService {
    /// Get all active requests, if any, from service chain
    fn get_requests(&self) -> Result<Option<Vec<ServiceRequest>>> {
        Ok(None)
    }

    /// Try get active request, by genesis hash, from service chain
    fn get_request(&self, hash: &sha256d::Hash) -> Result<Option<ServiceRequest>> {
        if self.return_none {
            return Ok(None);
        }
        if self.return_err {
            return Err(Error::from(CError::Generic("get_request failed".to_owned())));
        }

        let mut dummy_req = self.request.borrow_mut();
        dummy_req.genesis_blockhash = *hash;
        Ok(Some(dummy_req.clone()))
    }

    /// Try get active request bids, by transaction hash, from service chain
    fn get_request_bids(&self, _hash: &sha256d::Hash) -> Result<Option<BidSet>> {
        if self.return_none {
            return Ok(None);
        }
        if self.return_err {
            return Err(Error::from(CError::Generic("get_request_bids failed".to_owned())));
        }
        let mut bid_set = BidSet::new();
        let _ = bid_set.insert(Bid {
            txid: sha256d::Hash::from_hex("1234567890000000000000000000000000000000000000000000000000000000").unwrap(),
            // pubkey corresponding to SecretKey::from_slice(&[0xaa; 32])
            pubkey: PublicKey::from_str("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3").unwrap(),
            payment: None,
        });
        let _ = bid_set.insert(Bid {
            txid: sha256d::Hash::from_hex("0000000001234567890000000000000000000000000000000000000000000000").unwrap(),
            // pubkey corresponding to SecretKey::from_slice(&[0xbb; 32])
            pubkey: PublicKey::from_str("0268680737c76dabb801cb2204f57dbe4e4579e4f710cd67dc1b4227592c81e9b5").unwrap(),
            payment: None,
        });
        let _ = bid_set.insert(Bid {
            txid: sha256d::Hash::from_hex("0000000000000000001234567890000000000000000000000000000000000000").unwrap(),
            // pubkey corresponding to SecretKey::from_slice(&[0xcc; 32])
            pubkey: PublicKey::from_str("02b95c249d84f417e3e395a127425428b540671cc15881eb828c17b722a53fc599").unwrap(),
            payment: None,
        });
        Ok(Some(bid_set))
    }

    /// Get service chain blockheight
    fn get_blockheight(&self) -> Result<u64> {
        if self.return_err {
            return Err(Error::from(CError::Generic("get_blockheight failed".to_owned())));
        }

        let mut height = self.height.borrow_mut();
        *height += 1; // increment height for integration testing
        Ok(*height - 1) // return previous height
    }
}
