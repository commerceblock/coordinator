//! Mock storage
//!
//! Mock storage implementation for testing

use std::cell::RefCell;

use bitcoin::hashes::sha256d;
use mongodb::ordered::OrderedDocument;
use mongodb::Bson;

use crate::error::{CError, Error, Result};
use crate::interfaces::storage::*;
use crate::interfaces::{
    bid::{Bid, BidSet},
    request::Request as ServiceRequest,
    response::Response,
};
use crate::util::doc_format::*;

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
    fn save_challenge_request_state(&self, request: &ServiceRequest, bids: &BidSet) -> Result<()> {
        if self.return_err {
            return Err(Error::from(CError::Generic(
                "save_challenge_request_state failed".to_owned(),
            )));
        }
        // do not add request if already exists
        if !self
            .requests
            .borrow_mut()
            .iter()
            .any(|req_store| req_store.get("txid").unwrap().as_str().unwrap() == &request.txid.to_string())
        {
            self.requests.borrow_mut().push(request_to_doc(&request));
        }
        for bid in bids.iter() {
            self.bids
                .borrow_mut()
                .push(bid_to_doc(&Bson::String(request.txid.to_string()), bid))
        }
        Ok(())
    }

    /// update request in mock storage
    fn update_request(&self, request_update: &ServiceRequest) -> Result<()> {
        for request in self.requests.borrow_mut().iter_mut() {
            if request.get("txid").unwrap().as_str().unwrap() == &request_update.txid.to_string() {
                *request = request_to_doc(&request_update);
            }
        }
        Ok(())
    }

    /// update bid in mock storage
    fn update_bid(&self, _request_hash: sha256d::Hash, _bid: &Bid) -> Result<()> {
        Ok(())
    }

    /// Store response for a specific challenge request
    fn save_response(&self, request_hash: sha256d::Hash, response: &Response) -> Result<()> {
        if self.return_err {
            return Err(Error::from(CError::Generic("save_response failed".to_owned())));
        }

        for resp_doc in self.challenge_responses.borrow_mut().iter_mut() {
            if resp_doc.get("request_id").unwrap().as_str().unwrap() == &request_hash.to_string() {
                *resp_doc = response_to_doc(&Bson::String(request_hash.to_string()), &response);
                return Ok(());
            }
        }

        self.challenge_responses
            .borrow_mut()
            .push(response_to_doc(&Bson::String(request_hash.to_string()), &response));
        Ok(())
    }

    /// Get challenge response for a specific request
    fn get_response(&self, request_hash: sha256d::Hash) -> Result<Option<Response>> {
        for doc in self.challenge_responses.borrow().to_vec().iter() {
            if doc.get("request_id").unwrap().as_str().unwrap() == request_hash.to_string() {
                return Ok(Some(doc_to_response(doc)));
            }
        }
        Ok(None)
    }

    /// Get all bids for a specific request
    fn get_bids(&self, request_hash: sha256d::Hash) -> Result<Vec<Bid>> {
        let mut bids = Vec::new();
        for doc in self.bids.borrow().to_vec().iter() {
            if doc.get("request_id").unwrap().as_str().unwrap() == request_hash.to_string() {
                let _ = bids.push(doc_to_bid(doc));
            }
        }
        Ok(bids)
    }

    /// Get all the requests, with an optional flag to return payment complete
    /// only
    fn get_requests(
        &self,
        _complete: Option<bool>,
        limit: Option<i64>,
        skip: Option<i64>,
    ) -> Result<Vec<ServiceRequest>> {
        let skip_val = skip.unwrap_or(0);
        let limit_val = limit.unwrap_or(10000000);
        let mut requests = vec![];
        for (i, doc) in self.requests.borrow().to_vec().iter().enumerate() {
            if i as i64 >= skip_val && (requests.len() as i64) < limit_val {
                requests.push(doc_to_request(doc))
            }
        }
        Ok(requests)
    }

    /// Get the number of requests stored in memory
    fn get_requests_count(&self) -> Result<i64> {
        Ok(self.requests.borrow().len() as i64)
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
