//! # Testing Utils
//!
//! Colleciton of helper functions used in tests module

use bitcoin_hashes::{hex::FromHex, sha256d, Hash};
use mongodb::ordered::OrderedDocument;
use mongodb::Bson;
use secp256k1::PublicKey;
use std::cell::RefCell;
use std::str::FromStr;
use util::doc_format::*;

use crate::challenger::{ChallengeResponseIds, ChallengeState};
use crate::clientchain::ClientChain;
use crate::request::{Bid, BidSet, Request as ServiceRequest};
use crate::service::Service;
use crate::storage::*;

use crate::error::*;

/// Generate dummy hash
pub fn gen_dummy_hash(i: u8) -> sha256d::Hash {
    sha256d::Hash::from_slice(&[i as u8; 32]).unwrap()
}

/// Generate dummy challenge state
pub fn gen_challenge_state(request_hash: &sha256d::Hash) -> ChallengeState {
    let request = ServiceRequest {
        txid: *request_hash,
        start_blockheight: 2,
        end_blockheight: 5,
        genesis_blockhash: gen_dummy_hash(0),
        fee_percentage: 5,
        num_tickets: 10,
    };
    let mut bids = BidSet::new();
    let _ = bids.insert(Bid {
        txid: sha256d::Hash::from_hex("1234567890000000000000000000000000000000000000000000000000000000").unwrap(),
        pubkey: PublicKey::from_str("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3").unwrap(),
    });
    ChallengeState {
        request,
        bids,
        latest_challenge: Some(gen_dummy_hash(0)),
    }
}

/// Generate dummy challenge state with specific challenge
pub fn gen_challenge_state_with_challenge(
    request_hash: &sha256d::Hash,
    challenge_hash: &sha256d::Hash,
) -> ChallengeState {
    let request = ServiceRequest {
        txid: sha256d::Hash::from_slice(&[0xff as u8; 32]).unwrap(),
        start_blockheight: 2,
        end_blockheight: 5,
        genesis_blockhash: *request_hash,
        fee_percentage: 5,
        num_tickets: 10,
    };
    let mut bids = BidSet::new();
    let _ = bids.insert(Bid {
        txid: sha256d::Hash::from_hex("1234567890000000000000000000000000000000000000000000000000000000").unwrap(),
        // pubkey corresponding to SecretKey::from_slice(&[0xaa; 32])
        pubkey: PublicKey::from_str("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3").unwrap(),
    });
    ChallengeState {
        request,
        bids,
        latest_challenge: Some(*challenge_hash),
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
    /// Mock client chain blockheight
    pub height: RefCell<u64>,
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
}

/// Mock implementation of Service using some mock logic for testing
pub struct MockService {
    /// Flag that when set returns error on all inherited methods that return
    /// Result
    pub return_err: bool,
    /// Flag that when set returns None on all inherited methods that return
    /// Option
    pub return_none: bool,
    /// Current active request
    pub request: ServiceRequest,
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
        };

        MockService {
            return_err: false,
            return_none: false,
            request,
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

        let mut dummy_req = self.request.clone();
        dummy_req.genesis_blockhash = *hash;
        Ok(Some(dummy_req))
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
        });
        let _ = bid_set.insert(Bid {
            txid: sha256d::Hash::from_hex("0000000001234567890000000000000000000000000000000000000000000000").unwrap(),
            // pubkey corresponding to SecretKey::from_slice(&[0xbb; 32])
            pubkey: PublicKey::from_str("0268680737c76dabb801cb2204f57dbe4e4579e4f710cd67dc1b4227592c81e9b5").unwrap(),
        });
        let _ = bid_set.insert(Bid {
            txid: sha256d::Hash::from_hex("0000000000000000001234567890000000000000000000000000000000000000").unwrap(),
            // pubkey corresponding to SecretKey::from_slice(&[0xcc; 32])
            pubkey: PublicKey::from_str("02b95c249d84f417e3e395a127425428b540671cc15881eb828c17b722a53fc599").unwrap(),
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

/// Mock implementation of Storage storing data in memory for testing
#[derive(Debug)]
pub struct MockStorage {
    /// Flag that when set returns error on all inherited methods that return
    /// Result
    pub return_err: bool,
    /// Store requests in memory
    pub requests: RefCell<Vec<OrderedDocument>>,
    /// Store bids in memory
    pub bids: RefCell<Vec<OrderedDocument>>,
    /// Store challenge responses in memory
    pub challenge_responses: RefCell<Vec<OrderedDocument>>,
}

impl MockStorage {
    /// Create a MockStorage with all flags turned off by default
    pub fn new() -> Self {
        MockStorage {
            return_err: false,
            requests: RefCell::new(vec![]),
            bids: RefCell::new(vec![]),
            challenge_responses: RefCell::new(vec![]),
        }
    }
}

impl Storage for MockStorage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()> {
        if self.return_err {
            return Err(Error::from(CError::Generic("save_challenge_state failed".to_owned())));
        }
        self.requests.borrow_mut().push(request_to_doc(&challenge.request));
        for bid in challenge.bids.iter() {
            self.bids
                .borrow_mut()
                .push(bid_to_doc(&Bson::String(challenge.request.txid.to_string()), bid))
        }
        Ok(())
    }

    /// Store responses for a specific challenge request
    fn save_response(&self, request_hash: sha256d::Hash, ids: &ChallengeResponseIds) -> Result<()> {
        if self.return_err {
            return Err(Error::from(CError::Generic("save_response failed".to_owned())));
        }
        self.challenge_responses
            .borrow_mut()
            .push(challenge_responses_to_doc(&Bson::String(request_hash.to_string()), ids));
        Ok(())
    }

    /// Get all challenge responses for a specific request
    fn get_responses(&self, request_hash: sha256d::Hash) -> Result<Vec<ChallengeResponseIds>> {
        let mut challenge_responses = vec![];
        for doc in self.challenge_responses.borrow().to_vec().iter() {
            if doc.get("request_id").unwrap().as_str().unwrap() == request_hash.to_string() {
                challenge_responses.push(doc_to_challenge_responses(doc));
            }
        }
        Ok(challenge_responses)
    }

    /// Get all bids for a specific request
    fn get_bids(&self, request_hash: sha256d::Hash) -> Result<BidSet> {
        let mut bids = BidSet::new();
        for doc in self.bids.borrow().to_vec().iter() {
            if doc.get("request_id").unwrap().as_str().unwrap() == request_hash.to_string() {
                let _ = bids.insert(doc_to_bid(doc));
            }
        }
        Ok(bids)
    }

    /// Get all the requests
    fn get_requests(&self) -> Result<Vec<ServiceRequest>> {
        let mut requests = vec![];
        for doc in self.requests.borrow().to_vec().iter() {
            requests.push(doc_to_request(doc))
        }
        Ok(requests)
    }

    /// Get request for a specific request txid
    fn get_request(&self, request_hash: sha256d::Hash) -> Result<Option<ServiceRequest>> {
        for doc in self.requests.borrow().to_vec().iter() {
            if doc.get("txid").unwrap().as_str().unwrap() == request_hash.to_string() {
                return Ok(Some(doc_to_request(doc)));
            }
        }
        Ok(None)
    }
}
