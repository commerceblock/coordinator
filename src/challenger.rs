//! Challenger
//!
//! Methods and models for fetching, structuring and running challenge requests

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin_hashes::sha256d;

use crate::clientchain::ClientChain;
use crate::error::{CError, Error, Result};
use crate::request::{Bid, BidSet, Request};
use crate::service::Service;
use crate::storage::Storage;

/// Number of verify attempts for challenge transaction
pub const CHALLENGER_VERIFY_ATTEMPTS: u32 = 5;

/// Attempts to verify that a challenge has been included in the client chain
/// Method tries a fixed number of attempts CHALLENGER_VERIFY_ATTEMPTS for a
/// variable delay time to allow easy configuration
fn verify_challenge<K: ClientChain>(
    hash: &sha256d::Hash,
    clientchain: &K,
    attempt_delay: time::Duration,
) -> Result<bool> {
    info! {"verifying challenge hash: {}", hash}
    for i in 0..CHALLENGER_VERIFY_ATTEMPTS {
        // fixed number of attempts?
        if clientchain.verify_challenge(&hash)? {
            info! {"challenge verified"}
            return Ok(true);
        }
        warn! {"attempt {} failed", i+1}
        if i + 1 == CHALLENGER_VERIFY_ATTEMPTS {
            break;
        }
        info! {"sleeping for {:?}...", attempt_delay/CHALLENGER_VERIFY_ATTEMPTS}
        thread::sleep(attempt_delay / CHALLENGER_VERIFY_ATTEMPTS)
    }
    Ok(false)
}

/// Get responses to the challenge by reading data from the channel receiver
/// Channel is read for a configurable duration and then the method returns
/// all the responses that have been received for a specific challenge hash
fn get_challenge_response(
    challenge_hash: &sha256d::Hash,
    verify_rx: &Receiver<ChallengeResponse>,
    get_duration: time::Duration,
) -> Result<ChallengeResponseIds> {
    let mut responses = ChallengeResponseIds::new();

    let start_time = time::Instant::now();
    loop {
        let now = time::Instant::now();
        if start_time + get_duration > now {
            let duration = start_time + get_duration - now;
            match verify_rx.recv_timeout(duration) {
                Ok(resp) => {
                    if resp.0 == *challenge_hash {
                        // filter old invalid/responses
                        let _ = responses.insert(resp.1.txid);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {} // ignore timeout - it's allowed
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Error::from(CError::ReceiverDisconnected));
                }
            }
        } else {
            break;
        }
    }

    Ok(responses)
}

/// Run challenge for a specific request on the client chain. On each new
/// service height send a challenge on the client chain continuing until active
/// request expires (end_blockheight). For each challenge, verify it has been
/// included to the client chain and then fetch all challenge responses for a
/// specified time duration. These responses are then stored via the storage
/// interface
pub fn run_challenge_request<T: Service, K: ClientChain, D: Storage>(
    service: &T,
    clientchain: &K,
    challenge_state: Arc<Mutex<ChallengeState>>,
    verify_rx: &Receiver<ChallengeResponse>,
    storage: Arc<D>,
    verify_duration: time::Duration,
    challenge_duration: time::Duration,
    challenge_frequency: u64,
) -> Result<()> {
    let request = challenge_state.lock().unwrap().request.clone(); // clone as const and drop mutex
    info! {"Running challenge request: {:?}", request};
    let mut prev_challenge_height: u64 = 0;
    loop {
        let challenge_height = service.get_blockheight()?;
        info! {"service chain height: {}", challenge_height}
        if (request.end_blockheight as u64) < challenge_height {
            break;
        } else if (challenge_height - prev_challenge_height) < challenge_frequency {
            info! {"Sleeping for 10 sec..."}
            thread::sleep(time::Duration::from_secs(10));
            continue;
        }

        info! {"sending challenge..."}
        let challenge_hash = clientchain.send_challenge()?;
        challenge_state.lock().unwrap().latest_challenge = Some(challenge_hash);

        if !verify_challenge(&challenge_hash, clientchain, verify_duration)? {
            challenge_state.lock().unwrap().latest_challenge = None; // stop receiving responses
            continue;
        }

        info! {"fetching responses..."}
        storage.save_response(
            request.txid,
            &get_challenge_response(&challenge_hash, &verify_rx, challenge_duration)?,
        )?;
        challenge_state.lock().unwrap().latest_challenge = None; // stop receiving responses
        prev_challenge_height = challenge_height; // update prev height
    }
    info! {"Challenge request ended"}
    Ok(())
}

/// Tuple struct to store a verified challenge response
/// for a winning bid on a specific challenge hash
#[derive(Debug, Hash, Clone)]
pub struct ChallengeResponse(pub sha256d::Hash, pub Bid);

impl PartialEq for ChallengeResponse {
    fn eq(&self, other: &ChallengeResponse) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}
impl Eq for ChallengeResponse {}

/// Type defining a set of Challenge Responses Ids
pub type ChallengeResponseIds = HashSet<sha256d::Hash>;

/// Mainstains challenge state with information on
/// challenge requests and bids as well as the
/// latest challenge hash in the client chain
#[derive(Debug, Clone)]
pub struct ChallengeState {
    /// Service Request for issuing challenges
    pub request: Request,
    /// Request winning bids that respond to challenges
    pub bids: BidSet,
    /// Latest challenge txid hash in the client chain
    pub latest_challenge: Option<sha256d::Hash>,
}

/// Check if request start height has been reached in order to initiate
/// challenging.
fn check_request(request: &Request, height: u64) -> bool {
    return if request.start_blockheight as u64 <= height {
        true
    } else {
        false
    };
}

/// Attempt to fetch the winnings bids for a request in the service chain
fn get_request_bids<T: Service>(request: &Request, service: &T) -> Result<BidSet> {
    match service.get_request_bids(&request.txid)? {
        Some(bids) => return Ok(bids),
        _ => Err(Error::from(CError::MissingBids)),
    }
}

/// Fetch next challenge state given a request and bids in the service chain
/// A challenge is fetched only when a request exists and the required
/// starting blockheight has been reached in the service chain
pub fn fetch_next<T: Service>(service: &T, genesis: &sha256d::Hash) -> Result<Option<ChallengeState>> {
    info!("Fetching challenge request!");
    match service.get_request(&genesis)? {
        Some(req) => {
            let height = service.get_blockheight()?;
            if check_request(&req, height) {
                let bids = get_request_bids(&req, service)?;
                return Ok(Some(ChallengeState {
                    request: req,
                    bids: bids,
                    latest_challenge: None,
                }));
            } else {
                warn! {"Request (startheight: {}) not ready for current height: {}", req.start_blockheight, height}
            }
        }
        _ => {
            warn! {"No request found for genesis: {}.", genesis}
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc::{channel, Receiver, Sender};

    use bitcoin_hashes::Hash;

    use crate::clientchain::MockClientChain;
    use crate::error::Error;
    use crate::service::MockService;
    use crate::storage::MockStorage;

    /// Generate dummy hash for tests
    fn gen_dummy_hash(i: u8) -> sha256d::Hash {
        sha256d::Hash::from_slice(&[i as u8; 32]).unwrap()
    }

    #[test]
    fn verify_challenge_test() {
        let mut clientchain = MockClientChain::new();
        let dummy_hash = gen_dummy_hash(5);

        assert!(verify_challenge(&dummy_hash, &clientchain, time::Duration::from_nanos(1)).unwrap() == true);

        clientchain.return_false = true;
        assert!(verify_challenge(&dummy_hash, &clientchain, time::Duration::from_nanos(1)).unwrap() == false);
        clientchain.return_false = false;

        clientchain.return_err = true;
        assert!(
            verify_challenge(&dummy_hash, &clientchain, time::Duration::from_nanos(1)).is_err(),
            "verify_challenge failed"
        )
    }

    #[test]
    fn get_challenge_response_test() {
        let service = MockService::new();

        let dummy_hash = gen_dummy_hash(3);
        let dummy_bid = service
            .get_request_bids(&dummy_hash)
            .unwrap()
            .unwrap()
            .iter()
            .next()
            .unwrap()
            .clone();
        let (vtx, vrx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

        // first test with empty response
        let res = get_challenge_response(&dummy_hash, &vrx, time::Duration::from_millis(1));
        assert_eq!(res.unwrap().len(), 0);

        // then test with a few dummy responses and old hashes that are ignored
        let old_dummy_hash = gen_dummy_hash(8);
        let mut dummy_response_set = ChallengeResponseIds::new();
        let _ = dummy_response_set.insert(dummy_bid.txid);
        vtx.send(ChallengeResponse(dummy_hash, dummy_bid.clone())).unwrap();
        vtx.send(ChallengeResponse(dummy_hash, dummy_bid.clone())).unwrap();
        vtx.send(ChallengeResponse(old_dummy_hash, dummy_bid.clone())).unwrap();
        vtx.send(ChallengeResponse(dummy_hash, dummy_bid.clone())).unwrap();
        vtx.send(ChallengeResponse(old_dummy_hash, dummy_bid.clone())).unwrap();
        let res = get_challenge_response(&dummy_hash, &vrx, time::Duration::from_millis(1)).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res, dummy_response_set);

        // then drop channel sender and test correct error is returned
        std::mem::drop(vtx);
        let res = get_challenge_response(&dummy_hash, &vrx, time::Duration::from_millis(1));
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(Error::Coordinator(e)) => assert_eq!(CError::ReceiverDisconnected.to_string(), e.to_string()),
            Err(_) => assert!(false, "should not return any error"),
        }
    }

    #[test]
    fn check_request_test() {
        let service = MockService::new();
        let dummy_hash = gen_dummy_hash(11);
        let dummy_request = service.get_request(&dummy_hash).unwrap().unwrap();

        assert!(dummy_request.start_blockheight == 2);
        assert!(check_request(&dummy_request, 1) == false);
        assert!(check_request(&dummy_request, 3) == true);
        assert!(check_request(&dummy_request, 2) == true);
    }

    #[test]
    fn get_request_bids_test() {
        let mut service = MockService::new();
        let dummy_hash = gen_dummy_hash(10);
        let dummy_request = service.get_request(&dummy_hash).unwrap().unwrap();
        let dummy_set = service.get_request_bids(&dummy_hash).unwrap().unwrap();

        // first test with some bids
        let res = get_request_bids(&dummy_request, &service).unwrap();
        assert_eq!(res, dummy_set);

        // then test with None result
        service.return_none = true;
        let res = get_request_bids(&dummy_request, &service);
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(Error::Coordinator(e)) => assert_eq!(CError::MissingBids.to_string(), e.to_string()),
            Err(_) => assert!(false, "should not return any error"),
        }
        service.return_none = false;

        // then test with Err result
        service.return_err = true;
        let res = get_request_bids(&dummy_request, &service);
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(Error::Coordinator(e)) => assert_ne!(CError::MissingBids.to_string(), e.to_string()),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn fetch_next_test() {
        let dummy_hash = gen_dummy_hash(255);

        let mut service = MockService::new();
        let dummy_request = service.get_request(&dummy_hash).unwrap().unwrap();
        let dummy_set = service.get_request_bids(&dummy_hash).unwrap().unwrap();

        // first test what happens when service fails
        service.return_err = true;
        assert!(fetch_next(&service, &dummy_hash).is_err());
        service.return_err = false;

        // then test when get_request returns none
        service.return_none = true;
        let res = fetch_next(&service, &dummy_hash).unwrap();
        match res {
            None => assert!(true),
            Some(_) => assert!(false, "not expecting value"),
        }
        service.return_none = false;

        // then test when get_request returns Request
        let _ = service.height.replace(dummy_request.start_blockheight as u64);
        let res = fetch_next(&service, &dummy_hash).unwrap().unwrap();
        assert_eq!(res.latest_challenge, None);
        assert_eq!(res.bids, dummy_set);
        assert_eq!(res.request, dummy_request);

        // then test when get_request returns None as height too low
        let _ = service.height.replace(1);
        let res = fetch_next(&service, &dummy_hash).unwrap();
        match res {
            None => assert!(true),
            Some(_) => assert!(false, "not expecting value"),
        }
    }

    #[test]
    fn run_challenge_request_test() {
        let mut clientchain = MockClientChain::new();
        let mut storage = Arc::new(MockStorage::new());
        let mut service = MockService::new();

        let dummy_hash = gen_dummy_hash(0);
        let dummy_other_hash = gen_dummy_hash(9);
        let dummy_request = service.get_request(&dummy_hash).unwrap().unwrap();

        // test normal operation of run_challenge_request by adding some responses for
        // the first challenge
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height for fetch_next to succeed
        let challenge_state = fetch_next(&service, &dummy_hash).unwrap().unwrap();
        storage.save_challenge_state(&challenge_state).unwrap();

        let (vtx, vrx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

        let _ = clientchain.height.replace((dummy_request.start_blockheight as u64) + 1); // set height +1 for challenge hash response
        let dummy_challenge_hash = clientchain.send_challenge().unwrap();
        let dummy_bid = challenge_state.bids.iter().next().unwrap().clone();
        vtx.send(ChallengeResponse(dummy_challenge_hash, dummy_bid.clone()))
            .unwrap();

        // first test with large challenge frequency and observe that no responses are
        // fetched
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height back to starting height
        let res = run_challenge_request(
            &service,
            &clientchain,
            Arc::new(Mutex::new(challenge_state.clone())),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            3,
        );
        match res {
            Ok(_) => {
                let resps = storage.get_responses(dummy_request.txid).unwrap();
                assert_eq!(1, resps.len());
                let bids = storage.get_bids(dummy_request.txid).unwrap();
                assert_eq!(challenge_state.bids, bids);
                let requests = storage.get_requests().unwrap();
                assert_eq!(1, requests.len());
                assert_eq!(&challenge_state.request, &requests[0]);
                assert_eq!(
                    challenge_state.request,
                    storage.get_request(dummy_request.txid).unwrap().unwrap()
                );
                assert_eq!(None, storage.get_request(dummy_other_hash).unwrap());
            }
            Err(_) => assert!(false, "should not return error"),
        }

        // then test with normal frequency and observe that response is fetched
        vtx.send(ChallengeResponse(dummy_challenge_hash, dummy_bid.clone()))
            .unwrap(); // send again
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height back to starting height
        let res = run_challenge_request(
            &service,
            &clientchain,
            Arc::new(Mutex::new(challenge_state.clone())),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
        );
        match res {
            Ok(_) => {
                let resps = storage.get_responses(dummy_request.txid).unwrap();
                assert_eq!(5, resps.len());
                assert_eq!(1, resps[1].len());
                assert_eq!(dummy_bid.txid, *resps[1].iter().next().unwrap());
                assert_eq!(5, storage.challenge_responses.borrow().len());
                let bids = storage.get_bids(dummy_request.txid).unwrap();
                assert_eq!(challenge_state.bids, bids);
                let requests = storage.get_requests().unwrap();
                assert_eq!(1, requests.len());
                assert_eq!(&challenge_state.request, &requests[0]);
                assert_eq!(
                    challenge_state.request,
                    storage.get_request(dummy_request.txid).unwrap().unwrap()
                );
                assert_eq!(None, storage.get_request(dummy_other_hash).unwrap());
            }
            Err(_) => assert!(false, "should not return error"),
        }

        // test client chain failure
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height for fetch_next to succeed
        let challenge_state = fetch_next(&service, &dummy_hash).unwrap().unwrap();

        clientchain.return_err = true;
        assert!(run_challenge_request(
            &service,
            &clientchain,
            Arc::new(Mutex::new(challenge_state)),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
        )
        .is_err());
        clientchain.return_err = false;

        // test service chain failure
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height for fetch_next to succeed
        let challenge_state = fetch_next(&service, &dummy_hash).unwrap().unwrap();

        service.return_err = true;
        assert!(run_challenge_request(
            &service,
            &clientchain,
            Arc::new(Mutex::new(challenge_state)),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
        )
        .is_err());
        service.return_err = false;

        // test storage failure
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height for fetch_next to succeed
        let challenge_state = fetch_next(&service, &dummy_hash).unwrap().unwrap();

        let mut storage_err = MockStorage::new();
        storage_err.return_err = true;
        assert!(run_challenge_request(
            &service,
            &clientchain,
            Arc::new(Mutex::new(challenge_state)),
            &vrx,
            Arc::new(storage_err),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
        )
        .is_err());

        // test client chain returning false
        storage = Arc::new(MockStorage::new()); // reset storage;
        let _ = service.height.replace(dummy_request.start_blockheight as u64); // set height for fetch_next to succeed
        let challenge_state = fetch_next(&service, &dummy_hash).unwrap().unwrap();

        clientchain.return_false = true;
        vtx.send(ChallengeResponse(dummy_challenge_hash, dummy_bid.clone()))
            .unwrap();

        let res = run_challenge_request(
            &service,
            &clientchain,
            Arc::new(Mutex::new(challenge_state)),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
        );
        match res {
            Ok(_) => {
                assert_eq!(0, storage.challenge_responses.borrow().len());
            }
            Err(_) => assert!(false, "should not return error"),
        }
        clientchain.return_false = false;

        // test run when height is already passed
        storage = Arc::new(MockStorage::new()); // reset storage;
        let _ = service.height.replace(dummy_request.end_blockheight as u64 + 1); // set height for fetch_next to succeed
        let challenge_state = fetch_next(&service, &dummy_hash).unwrap().unwrap();

        vtx.send(ChallengeResponse(dummy_challenge_hash, dummy_bid.clone()))
            .unwrap();
        let res = run_challenge_request(
            &service,
            &clientchain,
            Arc::new(Mutex::new(challenge_state)),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
        );
        match res {
            Ok(_) => {
                assert_eq!(0, storage.challenge_responses.borrow().len());
            }
            Err(_) => assert!(false, "should not return error"),
        }
    }
}
