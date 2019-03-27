//! Storage
//!
//! Storage interface and implementations

use std::cell::RefCell;

use bitcoin_hashes::sha256d;
use mongodb::db::{Database, ThreadedDatabase};
use mongodb::{Bson, Client, ThreadedClient};
use serde_json::Value;

use crate::challenger::{ChallengeResponse, ChallengeResponseSet, ChallengeState};
use crate::error::{CError, Error, Result};

/// Storage trait defining required functionality for objects that store request
/// and challenge information
pub trait Storage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()>;
    /// Store responses for a specific challenge request
    fn save_challenge_responses(&self, request_hash: sha256d::Hash, responses: &ChallengeResponseSet) -> Result<()>;
    /// Get challenge responses for a specific request
    fn get_challenge_responses(&self, request_hash: sha256d::Hash) -> Result<Value>;
}

/// Database implementation of Storage trait
pub struct MongoStorage {
    db: Database,
}

impl MongoStorage {
    /// Create DbStorage instance
    pub fn new() -> Self {
        // TODO: add user/pass option
        let client = Client::with_uri("mongodb://localhost:27017/coordinator").expect("Failed to initialize client.");
        MongoStorage {
            db: client.db("coordinator"),
        }
    }
}

impl Storage for MongoStorage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()> {
        let request_id;
        let coll = self.db.collection("Request");
        let doc = doc! {
            "txid": challenge.request.txid.to_string(),
        };
        match coll.find_one(Some(doc.clone()), None)? {
            Some(res) => request_id = res.get("_id").unwrap().clone(),
            None => {
                request_id = coll.insert_one(doc.clone(), None)?.inserted_id.unwrap();
            }
        }

        let coll = self.db.collection("Bid");
        for bid in challenge.bids.iter() {
            let doc = doc! {
                "request_id": request_id.clone(),
                "txid": bid.txid.to_string(),
                "pubkey": bid.pubkey.to_string()
            };
            match coll.find_one(Some(doc.clone()), None)? {
                Some(_) => (),
                None => {
                    let _ = coll.insert_one(doc.clone(), None)?;
                }
            }
        }
        Ok(())
    }

    /// Store responses for a specific challenge request
    fn save_challenge_responses(&self, request_hash: sha256d::Hash, responses: &ChallengeResponseSet) -> Result<()> {
        let request = self
            .db
            .collection("Request")
            .find_one(
                Some(doc! {
                    "txid": request_hash.to_string(),
                }),
                None,
            )?
            .unwrap();
        let request_id = request.get("_id").unwrap().clone();

        let mut bids = vec![];
        for resp in responses.iter() {
            bids.push(Bson::String(resp.1.txid.to_string()));
        }

        let _ = self.db.collection("Response").insert_one(
            doc! {
                "request_id": request_id,
                "bid_txids": bids
            },
            None,
        )?;
        Ok(())
    }

    /// Get challenge responses for a specific request
    fn get_challenge_responses(&self, request_hash: sha256d::Hash) -> Result<Value> {
        let mut resp_aggr = self.db.collection("Request").aggregate(
            [
                doc! {
                    "$lookup": {
                        "from": "Response",
                        "localField": "_id",
                        "foreignField": "request_id",
                        "as": "challenges"
                    }
                },
                doc! {
                    "$match": {
                        "txid": request_hash.to_string()
                    }
                },
                doc! {
                    "$unwind": "$challenges"
                },
                doc! {
                    "$project": {
                        "_id": 0,
                        "challenge_txids": "$challenges.bid_txids"
                    }
                },
            ]
            .to_vec(),
            None,
        )?;

        let mut challenge_responses = vec![];
        while resp_aggr.has_next()? {
            let resp_value: Value = Bson::Document(resp_aggr.next().unwrap()?).into();
            challenge_responses.push(resp_value);
        }
        Ok(json!({ "challenge_responses": challenge_responses }))
    }
}

/// Mock implementation of Storage storing data in memory for testing
#[derive(Debug)]
pub struct MockStorage {
    /// Flag that when set returns error on all inherited methods that return
    /// Result
    pub return_err: bool,
    /// Store challenge states in memory
    pub challenge_states: RefCell<Vec<ChallengeState>>,
    /// Store challenge responses in memory
    pub challenge_responses: RefCell<Vec<ChallengeResponse>>,
}

impl MockStorage {
    /// Create a MockStorage with all flags turned off by default
    pub fn new() -> Self {
        MockStorage {
            return_err: false,
            challenge_states: RefCell::new(vec![]),
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
        self.challenge_states.borrow_mut().push(challenge.clone());
        Ok(())
    }

    /// Store responses for a specific challenge request
    fn save_challenge_responses(&self, _request_hash: sha256d::Hash, responses: &ChallengeResponseSet) -> Result<()> {
        if self.return_err {
            return Err(Error::from(CError::Generic(
                "save_challenge_responses failed".to_owned(),
            )));
        }
        self.challenge_responses.borrow_mut().extend(responses.clone());
        Ok(())
    }

    /// Get challenge responses for a specific request
    fn get_challenge_responses(&self, _request_hash: sha256d::Hash) -> Result<Value> {
        let mut challenge_responses = vec![];
        for resp in self.challenge_responses.borrow().to_vec().iter() {
            let resp_value = serde_json::Value::String(resp.1.txid.to_string());
            challenge_responses.push(resp_value);
        }
        Ok(json!({ "challenge_responses": challenge_responses }))
    }
}
