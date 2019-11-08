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
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()>;
    /// Update request DB entry
    fn update_request(&self, request: Request) -> Result<()>;
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
    fn save_challenge_state(&self, challenge: &ChallengeState) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;

        let request_id;
        let coll = db_locked.collection("Request");
        let filter = doc! {"txid"=>challenge.request.txid.to_string()};
        match coll.find_one(Some(filter), None)? {
            Some(res) => {
                request_id = res.get("_id").unwrap().clone();
            }
            None => {
                request_id = coll
                    .insert_one(request_to_doc(&challenge.request), None)?
                    .inserted_id
                    .unwrap();
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

    /// Update entry in Request collection with given Request model
    fn update_request(&self, request: Request) -> Result<()> {
        let db_locked = self.db.lock().unwrap();
        self.auth(&db_locked)?;
        let coll = db_locked.collection("Request");
        let filter = doc! {"txid"=>&request.txid.clone().to_string()};
        let update = doc! {"$set" => request_to_doc(&request)};
        let _ = coll.update_one(filter, update, None)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    use bitcoin_hashes::Hash;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::time;

    use crate::challenger::*;
    use crate::clientchain::ClientChain;
    use crate::request::Request;
    use crate::service::Service;
    use crate::storage::Storage;
    use crate::util::testing::{gen_dummy_hash, MockClientChain, MockService, MockStorage};

    #[test]
    fn set_request_clientchain_height_test() {
        let clientchain = MockClientChain::new();
        let storage = Arc::new(MockStorage::new());
        let service = MockService::new();

        // Make new request
        let dummy_hash = sha256d::Hash::from_slice(&[0xff as u8; 32]).unwrap();
        let dummy_request = service.get_request(&dummy_hash).unwrap().unwrap();
        assert_eq!(dummy_request.start_blockheight_clientchain, 0);
        assert_eq!(dummy_request.end_blockheight_clientchain, 0);

        // build dummy challenge state
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height for fetch_next to succeed
        let challenge_state = fetch_next(&service, &dummy_hash).unwrap().unwrap();
        storage.save_challenge_state(&challenge_state).unwrap();
        let (vtx, vrx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();
        let _ = clientchain.height.replace((dummy_request.start_blockheight) + 1); // set height +1 for challenge hash response
        let dummy_challenge_hash = clientchain.send_challenge().unwrap();
        let dummy_bid = challenge_state.bids.iter().next().unwrap().clone();
        vtx.send(ChallengeResponse(dummy_challenge_hash, dummy_bid.clone()))
            .unwrap();

        // test update_request
        assert_eq!(storage.get_requests().unwrap()[0].end_blockheight_clientchain, 0);
        let update_request = Request {
            txid: dummy_hash.clone(),
            start_blockheight: 2,
            end_blockheight: 5,
            genesis_blockhash: gen_dummy_hash(0),
            fee_percentage: 5,
            num_tickets: 10,
            start_blockheight_clientchain: 0,
            end_blockheight_clientchain: 10,
        };
        let _ = storage.update_request(update_request);
        assert_eq!(storage.get_requests().unwrap()[0].end_blockheight_clientchain, 10);

        // test request not added if already exists
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height back to starting height
        let _ = run_challenge_request(
            &service,
            &clientchain,
            Arc::new(Mutex::new(challenge_state.clone())),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            3,
            time::Duration::from_millis(10),
        );
        assert_eq!(storage.get_requests().unwrap().len(), 1);
    }
}
