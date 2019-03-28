//! Storage
//!
//! Storage interface and implementations

use std::cell::RefCell;

use bitcoin_hashes::sha256d;
use mongodb::db::{Database, ThreadedDatabase};
use mongodb::ordered::OrderedDocument;
use mongodb::{Bson, Client, ThreadedClient};

use crate::challenger::{ChallengeResponseIds, ChallengeState};
use crate::error::{CError, Error, Result};

/// Storage trait defining required functionality for objects that store request
/// and challenge information
pub trait Storage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()>;
    /// Store responses for a specific challenge request
    fn save_challenge_responses(&self, request_hash: sha256d::Hash, responses: &ChallengeResponseIds) -> Result<()>;
    /// Get all challenge responses for a specific request
    fn get_all_challenge_responses(&self, request_hash: sha256d::Hash) -> Result<Vec<ChallengeResponseIds>>;
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
    fn save_challenge_responses(&self, request_hash: sha256d::Hash, responses: &ChallengeResponseIds) -> Result<()> {
        if responses.len() == 0 {
            return Ok(());
        }

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

        let _ = self
            .db
            .collection("Response")
            .insert_one(challenge_responses_to_doc(request.get("_id").unwrap(), responses), None)?;
        Ok(())
    }

    /// Get all challenge responses for a specific request
    fn get_all_challenge_responses(&self, request_hash: sha256d::Hash) -> Result<Vec<ChallengeResponseIds>> {
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
            ]
            .to_vec(),
            None,
        )?;

        let mut all_resps: Vec<ChallengeResponseIds> = Vec::new();
        if let Some(resp) = resp_aggr.next() {
            for challenge in resp?.get_array("challenges").unwrap().iter() {
                all_resps.push(doc_to_challenge_responses(challenge.as_document().unwrap()))
            }
        }
        Ok(all_resps)
    }
}

/// Util method that generates a Response document from challenge responses
fn challenge_responses_to_doc(request_id: &Bson, responses: &ChallengeResponseIds) -> OrderedDocument {
    let bids = responses.iter().map(|x| Bson::String(x.clone())).collect::<Vec<_>>();
    doc! {
        "request_id": request_id.clone(),
        "bid_txids": bids
    }
}

/// Util method that generates challenge responses from a Response document
fn doc_to_challenge_responses(doc: &OrderedDocument) -> ChallengeResponseIds {
    doc.get_array("bid_txids")
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
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
    pub challenge_responses: RefCell<Vec<OrderedDocument>>,
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
    fn save_challenge_responses(&self, request_hash: sha256d::Hash, responses: &ChallengeResponseIds) -> Result<()> {
        if self.return_err {
            return Err(Error::from(CError::Generic(
                "save_challenge_responses failed".to_owned(),
            )));
        }
        if responses.len() == 0 {
            return Ok(());
        }
        self.challenge_responses.borrow_mut().push(challenge_responses_to_doc(
            &Bson::String(request_hash.to_string()),
            responses,
        ));
        Ok(())
    }

    /// Get all challenge responses for a specific request
    fn get_all_challenge_responses(&self, request_hash: sha256d::Hash) -> Result<Vec<ChallengeResponseIds>> {
        let mut challenge_responses = vec![];
        for doc in self.challenge_responses.borrow().to_vec().iter() {
            if doc.get("request_id").unwrap().as_str().unwrap() == request_hash.to_string() {
                challenge_responses.push(doc_to_challenge_responses(doc));
            }
        }
        Ok(challenge_responses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mongodb::oid::ObjectId;

    #[test]
    fn challenge_responses_doc_test() {
        let id = ObjectId::new().unwrap();
        let mut ids = ChallengeResponseIds::new();

        let doc = challenge_responses_to_doc(&Bson::ObjectId(id.clone()), &ids);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "bid_txids": []
            },
            doc
        );
        assert_eq!(ids, doc_to_challenge_responses(&doc));

        let _ = ids.insert("test".to_owned());
        let doc = challenge_responses_to_doc(&Bson::ObjectId(id.clone()), &ids);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "bid_txids": ["test"]
            },
            doc
        );
        assert_eq!(ids, doc_to_challenge_responses(&doc));

        let _ = ids.insert("test2".to_owned());
        let _ = ids.insert("test3".to_owned());
        let _ = ids.insert("test4".to_owned());
        let doc = challenge_responses_to_doc(&Bson::ObjectId(id.clone()), &ids);
        assert_eq!(&id, doc.get("request_id").unwrap().as_object_id().unwrap());
        for id in doc.get_array("bid_txids").unwrap().iter() {
            assert!(ids.contains(id.as_str().unwrap()));
        }
        assert_eq!(4, doc.get_array("bid_txids").unwrap().len());
        assert_eq!(ids, doc_to_challenge_responses(&doc));
    }
}
