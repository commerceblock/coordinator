//! Storage
//!
//! Storage interface and implementations

use std::cell::RefCell;
use std::str::FromStr;

use bitcoin_hashes::{hex::FromHex, sha256d};
use mongodb::db::{Database, ThreadedDatabase};
use mongodb::ordered::OrderedDocument;
use mongodb::{coll::options::FindOptions, Bson, Client, ThreadedClient};
use secp256k1::key::PublicKey;

use crate::challenger::{ChallengeResponseIds, ChallengeState};
use crate::config::StorageConfig;
use crate::error::{CError, Error, Result};
use crate::request::{Bid, BidSet};

/// Storage trait defining required functionality for objects that store request
/// and challenge information
pub trait Storage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()>;
    /// Store responses for a specific challenge request
    fn save_response(&self, request_hash: sha256d::Hash, ids: &ChallengeResponseIds) -> Result<()>;
    /// Get all challenge responses for a specific request
    fn get_responses(&self, request_hash: sha256d::Hash) -> Result<Vec<ChallengeResponseIds>>;
    /// Get all bids for a specific request
    fn get_bids(&self, request_hash: sha256d::Hash) -> Result<BidSet>;
    /// Get all the requests
    fn get_requests(&self) -> Result<Vec<sha256d::Hash>>;
}

/// Database implementation of Storage trait
pub struct MongoStorage {
    db: Database,
    config: StorageConfig,
}

impl MongoStorage {
    /// Create DbStorage instance
    pub fn new(storage_config: StorageConfig) -> Result<Self> {
        let uri = &format!("mongodb://{}/{}", storage_config.host, storage_config.name);
        let client = Client::with_uri(&uri)?;
        let db = client.db("coordinator");

        let mongo_storage = MongoStorage {
            db: db,
            config: storage_config,
        };
        mongo_storage.auth()?;
        let _ = mongo_storage.db.list_collections(None)?; // check connectivity
        Ok(mongo_storage)
    }

    /// Do db authentication using user/pass from config
    fn auth(&self) -> Result<()> {
        if let Some(ref user) = self.config.user {
            if let Some(ref pass) = self.config.pass {
                self.db.auth(user, pass)?;
            }
        }
        Ok(())
    }
}

impl Storage for MongoStorage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()> {
        self.auth()?;
        let request_id;
        let coll = self.db.collection("Request");
        let doc = request_to_doc(&challenge.request.txid);
        match coll.find_one(Some(doc.clone()), None)? {
            Some(res) => request_id = res.get("_id").unwrap().clone(),
            None => {
                request_id = coll.insert_one(doc, None)?.inserted_id.unwrap();
            }
        }

        let coll = self.db.collection("Bid");
        for bid in challenge.bids.iter() {
            let doc = bid_to_doc(&request_id, bid);
            match coll.find_one(Some(doc.clone()), None)? {
                Some(_) => (),
                None => {
                    let _ = coll.insert_one(doc, None)?;
                }
            }
        }
        Ok(())
    }

    /// Store responses for a specific challenge request
    fn save_response(&self, request_hash: sha256d::Hash, ids: &ChallengeResponseIds) -> Result<()> {
        self.auth()?;
        if ids.len() == 0 {
            return Ok(());
        }

        let request = self
            .db
            .collection("Request")
            .find_one(Some(request_to_doc(&request_hash)), None)?
            .unwrap();

        let _ = self
            .db
            .collection("Response")
            .insert_one(challenge_responses_to_doc(request.get("_id").unwrap(), ids), None)?;
        Ok(())
    }

    /// Get all challenge responses for a specific request
    fn get_responses(&self, request_hash: sha256d::Hash) -> Result<Vec<ChallengeResponseIds>> {
        self.auth()?;
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
                    "$match": request_to_doc(&request_hash)
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

    /// Get all bids for a specific request
    fn get_bids(&self, request_hash: sha256d::Hash) -> Result<BidSet> {
        self.auth()?;
        let mut resp_aggr = self.db.collection("Request").aggregate(
            [
                doc! {
                    "$lookup": {
                        "from": "Bid",
                        "localField": "_id",
                        "foreignField": "request_id",
                        "as": "bids"
                    }
                },
                doc! {
                    "$match": request_to_doc(&request_hash)
                },
            ]
            .to_vec(),
            None,
        )?;

        let mut all_bids = BidSet::new();
        if let Some(resp) = resp_aggr.next() {
            for bid in resp?.get_array("bids").unwrap().iter() {
                let _ = all_bids.insert(doc_to_bid(bid.as_document().unwrap()));
            }
        }
        Ok(all_bids)
    }

    /// Get all the requests
    fn get_requests(&self) -> Result<Vec<sha256d::Hash>> {
        self.auth()?;
        let mut options = FindOptions::new();
        options.sort = Some(doc! { "_id" : 1 }); // sort ascending, latest request is last
        let mut resps = self.db.collection("Request").find(None, Some(options))?;

        let mut requests = vec![];
        if let Some(resp) = resps.next() {
            requests.push(doc_to_request(&resp?))
        }
        Ok(requests)
    }
}

/// Util method that generates a Request document from a request
fn request_to_doc(request: &sha256d::Hash) -> OrderedDocument {
    doc! {
        "txid": request.to_string()
    }
}

/// Util method that generates a request from a Request document
fn doc_to_request(doc: &OrderedDocument) -> sha256d::Hash {
    sha256d::Hash::from_hex(doc.get("txid").unwrap().as_str().unwrap()).unwrap()
}

/// Util method that generates a Bid document from a request bid
fn bid_to_doc(request_id: &Bson, bid: &Bid) -> OrderedDocument {
    doc! {
        "request_id": request_id.clone(),
        "txid": bid.txid.to_string(),
        "pubkey": bid.pubkey.to_string()
    }
}

/// Util method that generates a request bid from a Bid document
fn doc_to_bid(doc: &OrderedDocument) -> Bid {
    Bid {
        txid: sha256d::Hash::from_hex(doc.get("txid").unwrap().as_str().unwrap()).unwrap(),
        pubkey: PublicKey::from_str(doc.get("pubkey").unwrap().as_str().unwrap()).unwrap(),
    }
}

/// Util method that generates a Response document from challenge responses
fn challenge_responses_to_doc(request_id: &Bson, responses: &ChallengeResponseIds) -> OrderedDocument {
    let bids = responses
        .iter()
        .map(|x| Bson::String(x.to_string()))
        .collect::<Vec<_>>();
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
        .map(|x| sha256d::Hash::from_hex(x.as_str().unwrap()).unwrap())
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
    fn save_response(&self, request_hash: sha256d::Hash, ids: &ChallengeResponseIds) -> Result<()> {
        if self.return_err {
            return Err(Error::from(CError::Generic("save_response failed".to_owned())));
        }
        if ids.len() == 0 {
            return Ok(());
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
        for state in self.challenge_states.borrow().to_vec().iter() {
            if state.request.txid == request_hash {
                return Ok(state.bids.clone());
            }
        }
        Ok(BidSet::new())
    }

    /// Get all the requests
    fn get_requests(&self) -> Result<Vec<sha256d::Hash>> {
        let mut requests = vec![];
        for state in self.challenge_states.borrow().to_vec().iter() {
            requests.push(state.request.txid)
        }
        Ok(requests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bitcoin_hashes::Hash;
    use mongodb::oid::ObjectId;

    /// Generate dummy hash for tests
    fn gen_dummy_hash(i: u8) -> sha256d::Hash {
        sha256d::Hash::from_slice(&[i as u8; 32]).unwrap()
    }

    #[test]
    fn request_doc_test() {
        let request = gen_dummy_hash(9);

        let doc = request_to_doc(&request);
        assert_eq!(
            doc! {
                "txid": request.to_string(),
            },
            doc
        );
        assert_eq!(request, doc_to_request(&doc));
    }

    #[test]
    fn bid_doc_test() {
        let id = ObjectId::new().unwrap();
        let pubkey_hex = "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3";
        let hash = gen_dummy_hash(1);
        let bid = Bid {
            txid: hash,
            pubkey: PublicKey::from_str(pubkey_hex).unwrap(),
        };

        let doc = bid_to_doc(&Bson::ObjectId(id.clone()), &bid);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "txid": hash.to_string(),
                "pubkey": pubkey_hex
            },
            doc
        );
        assert_eq!(bid, doc_to_bid(&doc));
    }

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

        let _ = ids.insert(gen_dummy_hash(0));
        let doc = challenge_responses_to_doc(&Bson::ObjectId(id.clone()), &ids);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "bid_txids": [gen_dummy_hash(0).to_string()]
            },
            doc
        );
        assert_eq!(ids, doc_to_challenge_responses(&doc));

        let _ = ids.insert(gen_dummy_hash(1));
        let _ = ids.insert(gen_dummy_hash(2));
        let _ = ids.insert(gen_dummy_hash(3));
        let doc = challenge_responses_to_doc(&Bson::ObjectId(id.clone()), &ids);
        assert_eq!(&id, doc.get("request_id").unwrap().as_object_id().unwrap());
        for id in doc.get_array("bid_txids").unwrap().iter() {
            assert!(ids.contains(&sha256d::Hash::from_hex(id.as_str().unwrap()).unwrap()));
        }
        assert_eq!(4, doc.get_array("bid_txids").unwrap().len());
        assert_eq!(ids, doc_to_challenge_responses(&doc));
    }
}
