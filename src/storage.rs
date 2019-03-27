//! Storage
//!
//! Storage interface and implementations

use std::cell::RefCell;

use mongodb::db::{Database, ThreadedDatabase};
use mongodb::{Client, ThreadedClient};

use crate::challenger::{ChallengeResponse, ChallengeResponseSet, ChallengeState};
use crate::error::{CError, Error, Result};

/// Storage trait defining required functionality for objects that store request
/// and challenge information
pub trait Storage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()>;
    /// Store responses to a specific challenge request
    fn save_challenge_responses(&self, responses: &ChallengeResponseSet) -> Result<()>;
    /// Get challenge responses for a specific request
    fn get_challenge_responses(&self, challenge: &ChallengeState) -> Result<Vec<ChallengeResponse>>;
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
        let coll = self.db.collection("request");
        let doc = doc! {
            "txid": challenge.request.txid.to_string(),
        };
        match coll.find_one(Some(doc.clone()), None)? {
            Some(res) => request_id = res.get("_id").unwrap().clone(),
            None => {
                println!("request inserting...");
                let res = coll.insert_one(doc.clone(), None)?;
                request_id = res.inserted_id.unwrap();
            }
        }

        let coll = self.db.collection("bid");
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

    /// Store responses to a specific challenge request
    fn save_challenge_responses(&self, responses: &ChallengeResponseSet) -> Result<()> {
        Ok(())
    }

    /// Get challenge responses for a specific request
    fn get_challenge_responses(&self, _challenge: &ChallengeState) -> Result<Vec<ChallengeResponse>> {
        Ok(vec![])
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

    /// Store responses to a specific challenge request
    fn save_challenge_responses(&self, responses: &ChallengeResponseSet) -> Result<()> {
        if self.return_err {
            return Err(Error::from(CError::Generic(
                "save_challenge_responses failed".to_owned(),
            )));
        }
        self.challenge_responses.borrow_mut().extend(responses.clone());
        Ok(())
    }

    /// Get challenge responses for a specific request
    fn get_challenge_responses(&self, _challenge: &ChallengeState) -> Result<Vec<ChallengeResponse>> {
        Ok(self.challenge_responses.borrow().to_vec())
    }
}
