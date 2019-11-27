//! Storage
//!
//! Storage interface and implementations

use std::mem::drop;
use std::sync::{Mutex, MutexGuard};

use bitcoin::hashes::sha256d;
use mongodb::db::{Database, ThreadedDatabase};
use mongodb::{
    coll::options::{FindOptions, UpdateOptions},
    Client, ThreadedClient,
};

use crate::config::StorageConfig;
use crate::error::{Error::MongoDb, Result};
use crate::interfaces::response::Response;
use crate::interfaces::{
    bid::{Bid, BidSet},
    request::Request,
};
use crate::util::doc_format::*;

/// Storage trait defining required functionality for objects that store request
/// and challenge information
pub trait Storage {
    /// Store the state of a challenge request
    fn save_challenge_request_state(&self, request: &Request, bids: &BidSet) -> Result<()>;
    /// Update request in storage
    fn update_request(&self, request: &Request) -> Result<()>;
    /// Update bid in storage
    fn update_bid(&self, request_hash: sha256d::Hash, bid: &Bid) -> Result<()>;
    /// Store response for a specific challenge request
    fn save_response(&self, request_hash: sha256d::Hash, response: &Response) -> Result<()>;
    /// Get challenge response for a specific request
    fn get_response(&self, request_hash: sha256d::Hash) -> Result<Option<Response>>;
    /// Get all bids for a specific request
    fn get_bids(&self, request_hash: sha256d::Hash) -> Result<Vec<Bid>>;
    /// Get all the requests, with an optional flag to return payment complete
    /// only
    fn get_requests(&self, complete: Option<bool>, limit: Option<i64>, skip: Option<i64>) -> Result<Vec<Request>>;
    /// Get the number of requests in storage
    fn get_requests_count(&self) -> Result<i64>;
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

        // Specify collections Indexes
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
    fn save_challenge_request_state(&self, request: &Request, bids: &BidSet) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let request_id;
        let coll = db_locked.collection("Request");
        let filter = doc! {"txid"=>request.txid.to_string()};
        match coll.find_one(Some(filter), None)? {
            Some(res) => {
                request_id = res.get("_id").unwrap().clone();
            }
            None => {
                request_id = coll.insert_one(request_to_doc(&request), None)?.inserted_id.unwrap();
            }
        }

        let coll = db_locked.collection("Bid");
        for bid in bids.iter() {
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

    /// Update entry in Request collection with given Request object
    fn update_request(&self, request: &Request) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;
        let coll = db_locked.collection("Request");
        let filter = doc! {"txid"=>&request.txid.clone().to_string()};
        let update = doc! {"$set" => request_to_doc(&request)};
        let _ = coll.update_one(filter, update, None)?;
        Ok(())
    }

    /// Update entry in Bid collection with given Bid object
    fn update_bid(&self, request_hash: sha256d::Hash, bid: &Bid) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let request_id = db_locked
            .collection("Request")
            .find_one(
                Some(doc! {
                    "txid": request_hash.to_string(),
                }),
                None,
            )?
            .unwrap()
            .get("_id")
            .unwrap()
            .clone();

        let coll = db_locked.collection("Bid");
        let filter = doc! {"request_id": request_id.clone(), "txid": bid.txid.to_string()};
        let update = doc! {"$set" => bid_to_doc(&request_id, &bid)};
        let _ = coll.update_one(filter, update, None)?;
        Ok(())
    }

    /// Store response for a specific challenge request
    fn save_response(&self, request_hash: sha256d::Hash, response: &Response) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let request_id = db_locked
            .collection("Request")
            .find_one(
                Some(doc! {
                    "txid": request_hash.to_string(),
                }),
                None,
            )?
            .unwrap()
            .get("_id")
            .unwrap()
            .clone();

        let coll = db_locked.collection("Response");
        let filter = doc! {"request_id": request_id.clone()};
        let update = doc! {"$set" => response_to_doc(&request_id, &response)};
        let options = UpdateOptions {
            upsert: Some(true),
            ..Default::default()
        };
        let _ = coll.update_one(filter, update, Some(options))?;
        Ok(())
    }

    /// Get challenge response for a specific request
    fn get_response(&self, request_hash: sha256d::Hash) -> Result<Option<Response>> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let mut resp_aggr = db_locked.collection("Request").aggregate(
            [
                doc! {
                    "$lookup": {
                        "from": "Response",
                        "localField": "_id",
                        "foreignField": "request_id",
                        "as": "response"
                    }
                },
                doc! {
                    "$match": {
                        "txid": request_hash.to_string()
                    },
                },
                doc! {
                    "$unwind": {
                        "path": "$response"
                    }
                },
            ]
            .to_vec(),
            None,
        )?;
        drop(db_locked); // drop immediately on get requests

        if let Some(resp) = resp_aggr.next() {
            return Ok(Some(doc_to_response(&resp?.get_document("response").unwrap())));
        }
        Ok(None)
    }

    /// Get all bids for a specific request
    fn get_bids(&self, request_hash: sha256d::Hash) -> Result<Vec<Bid>> {
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

        let mut all_bids = Vec::new();
        if let Some(resp) = resp_aggr.next() {
            for bid in resp?.get_array("bids").unwrap().iter() {
                let _ = all_bids.push(doc_to_bid(bid.as_document().unwrap()));
            }
        }
        Ok(all_bids)
    }

    /// Get all the requests, with an optional flag to return payment complete
    /// only
    fn get_requests(&self, complete: Option<bool>, limit: Option<i64>, skip: Option<i64>) -> Result<Vec<Request>> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let mut options = FindOptions::new();
        options.sort = Some(doc! { "_id" : 1 }); // sort ascending, latest request is last
        options.limit = limit; // limit the number of returned requests
        options.skip = skip; // number of requests to skip
        let filter = if let Some(is_complete) = complete {
            Some(doc! { "is_payment_complete": is_complete })
        } else {
            None
        };
        let resps = db_locked.collection("Request").find(filter, Some(options))?;
        drop(db_locked); // drop immediately on get requests

        let mut requests = vec![];
        for resp in resps {
            if let Ok(req) = resp {
                requests.push(doc_to_request(&req))
            }
        }
        Ok(requests)
    }

    /// Get the number of requests in the Request collection
    fn get_requests_count(&self) -> Result<i64> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;
        Ok(db_locked.collection("Request").count(None, None)?)
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
