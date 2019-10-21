//! Storage
//!
//! Storage interface and implementations

use std::cell::RefCell;
use std::mem::drop;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use bitcoin_hashes::{hex::FromHex, sha256d};
use mongodb::db::{Database, ThreadedDatabase};
use mongodb::ordered::OrderedDocument;
use mongodb::{coll::options::FindOptions, Bson, Client, ThreadedClient};
use secp256k1::key::PublicKey;

use crate::challenger::{ChallengeResponseIds, ChallengeState};
use crate::config::StorageConfig;
use crate::error::{CError, Error, Result};
use crate::request::{Bid, BidSet, Request};

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
    fn get_requests(&self) -> Result<Vec<Request>>;
    /// Get request for a specific request txid
    fn get_request(&self, request_hash: sha256d::Hash) -> Result<Option<Request>>;
}

/// Database implementation of Storage trait
pub struct MongoStorage {
    db: Mutex<Database>,
    config: StorageConfig,
}

impl MongoStorage {
    /// Create DbStorage instance
    pub fn new(storage_config: StorageConfig) -> Result<Self> {
        let uri = &format!("mongodb://{}/{}", storage_config.host, storage_config.name);
        let client = Client::with_uri(&uri)?;

        let db = client.db("coordinator");
        if let Some(ref user) = storage_config.user {
            if let Some(ref pass) = storage_config.pass {
                db.auth(user, pass)?;
            }
        }

        Ok(MongoStorage {
            db: Mutex::new(db),
            config: storage_config,
        })
    }

    /// Do db authentication using user/pass from config
    fn auth(&self, db_locked: &MutexGuard<Database>) -> Result<()> {
        match db_locked.list_collections(None) {
            // only do authentication if connectivity check fails
            Err(_) => {
                if let Some(ref user) = self.config.user {
                    if let Some(ref pass) = self.config.pass {
                        db_locked.auth(user, pass)?;
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }
}

impl Storage for MongoStorage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let request_id;
        let coll = db_locked.collection("Request");
        let doc = request_to_doc(&challenge.request);
        match coll.find_one(Some(doc.clone()), None)? {
            Some(res) => request_id = res.get("_id").unwrap().clone(),
            None => {
                request_id = coll.insert_one(doc, None)?.inserted_id.unwrap();
            }
        }

        let coll = db_locked.collection("Bid");
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
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let request = db_locked
            .collection("Request")
            .find_one(
                Some(doc! {
                    "txid": request_hash.to_string(),
                }),
                None,
            )?
            .unwrap();

        let _ = db_locked
            .collection("Response")
            .insert_one(challenge_responses_to_doc(request.get("_id").unwrap(), ids), None)?;
        Ok(())
    }

    /// Get all challenge responses for a specific request
    fn get_responses(&self, request_hash: sha256d::Hash) -> Result<Vec<ChallengeResponseIds>> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let mut resp_aggr = db_locked.collection("Request").aggregate(
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
                    },
                },
            ]
            .to_vec(),
            None,
        )?;
        drop(db_locked); // drop immediately on get requests

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
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let mut resp_aggr = db_locked.collection("Request").aggregate(
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
                    "$match": {
                        "txid": request_hash.to_string()
                    },
                },
            ]
            .to_vec(),
            None,
        )?;
        drop(db_locked); // drop immediately on get requests

        let mut all_bids = BidSet::new();
        if let Some(resp) = resp_aggr.next() {
            for bid in resp?.get_array("bids").unwrap().iter() {
                let _ = all_bids.insert(doc_to_bid(bid.as_document().unwrap()));
            }
        }
        Ok(all_bids)
    }

    /// Get all the requests
    fn get_requests(&self) -> Result<Vec<Request>> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let mut options = FindOptions::new();
        options.sort = Some(doc! { "_id" : 1 }); // sort ascending, latest request is last
        let resps = db_locked.collection("Request").find(None, Some(options))?;
        drop(db_locked); // drop immediately on get requests

        let mut requests = vec![];
        for resp in resps {
            if let Ok(req) = resp {
                requests.push(doc_to_request(&req))
            }
        }
        Ok(requests)
    }

    /// Get request for a specific request txid
    fn get_request(&self, request_hash: sha256d::Hash) -> Result<Option<Request>> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let request = db_locked.collection("Request").find_one(
            Some(doc! {
                "txid": request_hash.to_string(),
            }),
            None,
        )?;
        drop(db_locked); // drop immediately on get requests

        match request {
            Some(doc) => Ok(Some(doc_to_request(&doc))),
            None => Ok(None),
        }
    }
}

/// Util method that generates a Request document from a request
fn request_to_doc(request: &Request) -> OrderedDocument {
    doc! {
        "txid": request.txid.to_string(),
        "start_blockheight": request.start_blockheight,
        "end_blockheight": request.end_blockheight,
        "genesis_blockhash": request.genesis_blockhash.to_string(),
        "fee_percentage": request.fee_percentage,
        "num_tickets": request.num_tickets
    }
}

/// Util method that generates a request from a Request document
fn doc_to_request(doc: &OrderedDocument) -> Request {
    Request {
        txid: sha256d::Hash::from_hex(doc.get("txid").unwrap().as_str().unwrap()).unwrap(),
        start_blockheight: doc.get("start_blockheight").unwrap().as_i32().unwrap() as u32,
        end_blockheight: doc.get("end_blockheight").unwrap().as_i32().unwrap() as u32,
        genesis_blockhash: sha256d::Hash::from_hex(doc.get("genesis_blockhash").unwrap().as_str().unwrap()).unwrap(),
        fee_percentage: doc.get("fee_percentage").unwrap().as_i32().unwrap() as u32,
        num_tickets: doc.get("num_tickets").unwrap().as_i32().unwrap() as u32,
    }
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
    fn get_requests(&self) -> Result<Vec<Request>> {
        let mut requests = vec![];
        for doc in self.requests.borrow().to_vec().iter() {
            requests.push(doc_to_request(doc))
        }
        Ok(requests)
    }

    /// Get request for a specific request txid
    fn get_request(&self, request_hash: sha256d::Hash) -> Result<Option<Request>> {
        for doc in self.requests.borrow().to_vec().iter() {
            if doc.get("txid").unwrap().as_str().unwrap() == request_hash.to_string() {
                return Ok(Some(doc_to_request(doc)));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mongodb::oid::ObjectId;

    use crate::testing_utils::gen_dummy_hash;

    #[test]
    fn request_doc_test() {
        let request_hash = gen_dummy_hash(9);
        let genesis_hash = "1100000000000000000000000000000000000000000000000000000000000022";
        let request = Request {
            txid: request_hash,
            start_blockheight: 2,
            end_blockheight: 5,
            genesis_blockhash: sha256d::Hash::from_hex(genesis_hash).unwrap(),
            fee_percentage: 5,
            num_tickets: 10,
        };

        let doc = request_to_doc(&request);
        assert_eq!(
            doc! {
                "txid": request_hash.to_string(),
                "start_blockheight": 2,
                "end_blockheight": 5,
                "genesis_blockhash": genesis_hash,
                "fee_percentage": 5,
                "num_tickets": 10
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
