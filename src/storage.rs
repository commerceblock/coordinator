//! Storage
//!
//! Storage interface and implementations

use std::mem::drop;
use std::sync::{Mutex, MutexGuard};

use bitcoin_hashes::sha256d;
use mongodb::db::{Database, ThreadedDatabase};
use mongodb::{coll::options::FindOptions, Client, ThreadedClient};
use util::doc_format::*;

use crate::challenger::{ChallengeResponseIds, ChallengeState};
use crate::config::StorageConfig;
use crate::error::{Error::MongoDb, Result};
use crate::request::{BidSet, Request};

/// Storage trait defining required functionality for objects that store request
/// and challenge information
pub trait Storage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: &ChallengeState, cli_chain_height: u32) -> Result<()>;
    /// Set end_blockheight_cli in Request table if not already set
    fn set_end_blockheight_cli(&self, txid: String, cli_chain_height: u32) -> Result<()>;
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

        // Specify Indexes
        if let Err(e) = db.collection("Request").create_index(doc! ("txid":1), None) {
            return Err(MongoDb(e));
        }
        if let Err(e) = db.collection("Bid").create_index(doc! ("request_id":1), None) {
            return Err(MongoDb(e));
        }
        if let Err(e) = db.collection("Response").create_index(doc! ("request_id":1), None) {
            return Err(MongoDb(e));
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
    fn save_challenge_state(&self, challenge: &ChallengeState, cli_chain_height: u32) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let request_id;
        let coll = db_locked.collection("Request");
        let mut doc = request_to_doc(&challenge.request);
        println!("doc: {}", doc);
        let filter = doc! {"txid"=>doc.get_str("txid").unwrap()};
        match coll.find_one(Some(filter), None)? {
            Some(res) => {
                println!("res: {}", res);
                request_id = res.get("_id").unwrap().clone();
            }
            None => {
                // Set start_blockheight_cli if new to DB
                let _ = doc.insert("start_blockheight_cli", cli_chain_height);
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

    /// Set end_blockheight_cli in Request table if not already set
    fn set_end_blockheight_cli(&self, txid: String, cli_chain_height: u32) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;
        let coll = db_locked.collection("Request");
        let filter = doc! {"txid"=>txid.clone()};
        let update = doc! {"$set" => {"end_blockheight_cli"=>cli_chain_height}};
        match coll.find_one_and_update(filter, update, None)? {
            Some(res) => println!("update: {}", res),
            None => warn!(
                "Failed to find Request collection entry for end_blockheight_cli update. Request txid: {}.",
                txid
            ),
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
