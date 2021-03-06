//! Challenger
//!
//! Methods and models for fetching, structuring, storing and running challenge
//! requests

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, RwLock};
use std::{thread, time};

use bitcoin::hashes::sha256d;

use crate::error::{CError, Error, Result};
use crate::interfaces::clientchain::ClientChain;
use crate::interfaces::service::Service;
use crate::interfaces::storage::Storage;
use crate::interfaces::{
    bid::{Bid, BidSet},
    request::Request,
    response::Response,
};

/// Verify attempt interval to client in ms
pub const CHALLENGER_VERIFY_INTERVAL: u64 = 100;

/// Attempts to verify that a challenge has been included in the client chain
/// This makes attempts every CHALLENGER_VERIFY_INTERVAL ms and for the verify
/// duration specified, which is variable in order to allow easy configuration
fn verify_challenge<K: ClientChain>(
    hash: &sha256d::Hash,
    clientchain: &K,
    verify_duration: time::Duration,
) -> Result<()> {
    info! {"verifying challenge hash: {}", hash}
    let start_time = time::Instant::now();
    loop {
        let now = time::Instant::now();
        if start_time + verify_duration > now {
            if clientchain.verify_challenge(&hash)? {
                info! {"challenge verified"}
                return Ok(());
            }
        } else {
            break;
        }
        // This will potentially be replaced by subscribing to the ocean node
        // for transaction updates but this is good enough for now
        thread::sleep(std::time::Duration::from_millis(CHALLENGER_VERIFY_INTERVAL))
    }
    Err(Error::from(CError::UnverifiedChallenge))
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
    challenge_state: Arc<RwLock<Option<ChallengeState>>>,
    verify_rx: &Receiver<ChallengeResponse>,
    storage: Arc<D>,
    verify_duration: time::Duration,
    challenge_duration: time::Duration,
    challenge_frequency: u64,
    refresh_delay: time::Duration,
) -> Result<()> {
    let request = challenge_state.read().unwrap().as_ref().unwrap().request.clone(); // clone as const and drop mutex
    let mut response = storage.get_response(request.txid)?.unwrap_or(Response::new());
    info! {"Running challenge request: {:?}", request.txid};
    let mut prev_challenge_height: u64 = 0;
    loop {
        let challenge_height = service.get_blockheight()?;
        info! {"service chain height: {}", challenge_height}
        if (request.end_blockheight as u64) < challenge_height {
            break;
        } else if (challenge_height - prev_challenge_height) < challenge_frequency {
            info! {"Sleeping for {} sec...",time::Duration::as_secs(&refresh_delay)}
            thread::sleep(refresh_delay);
            continue;
        }

        info! {"sending challenge..."}
        let challenge_hash = clientchain.send_challenge()?;
        challenge_state.write().unwrap().as_mut().unwrap().latest_challenge = Some(challenge_hash);

        if let Err(e) = verify_challenge(&challenge_hash, clientchain, verify_duration) {
            challenge_state.write().unwrap().as_mut().unwrap().latest_challenge = None; // stop receiving responses
            return Err(e);
        }

        info! {"fetching responses..."}
        response.update(&get_challenge_response(
            &challenge_hash,
            &verify_rx,
            challenge_duration,
        )?);
        storage.save_response(request.txid, &response)?;
        challenge_state.write().unwrap().as_mut().unwrap().latest_challenge = None; // stop receiving responses
        prev_challenge_height = challenge_height; // update prev height
    }
    info! {"Challenge request ended"}
    Ok(())
}

/// Update challenge state request with client chain start and end block
/// heights and store challenge state
/// If request already stored set challenge state request to request in
/// storage (catcher for coordinator failure after storing request but
/// before request service period over) and consider any offets between the
/// client and service chain This should only be used on an active servive
/// request challenge
pub fn update_challenge_request_state<K: ClientChain, S: Service, D: Storage>(
    clientchain: &K,
    service: &S,
    storage: Arc<D>,
    challenge: &mut ChallengeState,
    block_time_servicechain: u64,
    block_time_clientchain: u64,
) -> Result<()> {
    match storage.get_request(challenge.request.txid)? {
        Some(req) => {
            challenge.request = req;
            let service_height = service.get_blockheight()? as u32;
            let client_height = clientchain.get_blockheight()?;
            // Checking that nodes are synced correctly - just a precaution
            if service_height >= challenge.request.start_blockheight
                && client_height >= challenge.request.start_blockheight_clientchain
            {
                // get theoretical end clientchain height
                let service_period_time_s = (challenge.request.end_blockheight - challenge.request.start_blockheight)
                    * block_time_servicechain as u32;
                let client_end_height = challenge.request.start_blockheight_clientchain
                    + (service_period_time_s as f32 / block_time_clientchain as f32).floor() as u32;

                // get time passed in s since start of the service for both service/client
                let service_current_time_s =
                    (service_height - challenge.request.start_blockheight) * block_time_servicechain as u32;
                let client_current_time_s =
                    (client_height - challenge.request.start_blockheight_clientchain) * block_time_clientchain as u32;

                // calculate and apply the difference
                let time_diff_s = service_current_time_s as i32 - client_current_time_s as i32;
                if time_diff_s > 0 {
                    challenge.request.end_blockheight_clientchain =
                        client_end_height - time_diff_s as u32 / block_time_clientchain as u32;
                    info!(
                        "Request client chain end height updated to {}",
                        challenge.request.end_blockheight_clientchain
                    );
                    storage.update_request(&challenge.request)?;
                } else if time_diff_s < 0 {
                    challenge.request.end_blockheight_clientchain =
                        client_end_height + time_diff_s.abs() as u32 / block_time_clientchain as u32;
                    storage.update_request(&challenge.request)?;
                    info!(
                        "Request client chain end height updated to {}",
                        challenge.request.end_blockheight_clientchain
                    );
                }
            }
        }
        None => {
            // Set request's start_blockheight_clientchain
            challenge.request.start_blockheight_clientchain = clientchain.get_blockheight()?;
            info!(
                "Request client chain start height set to {}",
                challenge.request.start_blockheight_clientchain
            );

            // Calculate and set request's end_blockheight_clientchain
            let service_period_time_s = (challenge.request.end_blockheight - challenge.request.start_blockheight)
                * block_time_servicechain as u32;
            challenge.request.end_blockheight_clientchain = challenge.request.start_blockheight_clientchain
                + (service_period_time_s as f32 / block_time_clientchain as f32).floor() as u32;
            info!(
                "Request client chain end height set to {}",
                challenge.request.end_blockheight_clientchain
            );

            // Store Challenge Request and Bids
            storage.save_challenge_request_state(&challenge.request, &challenge.bids)?;
        }
    }
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

    use std::collections::HashSet;
    use std::iter::FromIterator;
    use std::sync::mpsc::{channel, Receiver, Sender};

    use crate::error::Error;
    use crate::interfaces::mocks::clientchain::MockClientChain;
    use crate::interfaces::mocks::service::MockService;
    use crate::interfaces::mocks::storage::MockStorage;
    use crate::interfaces::response::Response;
    use crate::util::testing::{gen_challenge_state, gen_dummy_hash, setup_logger};

    #[test]
    fn verify_challenge_test() {
        setup_logger();
        let mut clientchain = MockClientChain::new();
        let dummy_hash = gen_dummy_hash(5);

        // duration doesn't matter here
        assert!(verify_challenge(&dummy_hash, &clientchain, time::Duration::from_millis(10)).unwrap() == ());

        // test that for very small duration this fails
        let res = verify_challenge(&dummy_hash, &clientchain, time::Duration::from_nanos(1));
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(Error::Coordinator(e)) => assert_eq!(CError::UnverifiedChallenge.to_string(), e.to_string()),
            Err(_) => assert!(false, "should not return any error"),
        }

        // test with clientchain returning false
        clientchain.return_false = true;
        let res = verify_challenge(&dummy_hash, &clientchain, time::Duration::from_millis(10));
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(Error::Coordinator(e)) => assert_eq!(CError::UnverifiedChallenge.to_string(), e.to_string()),
            Err(_) => assert!(false, "should not return any error"),
        }
        clientchain.return_false = false;

        // test with clientchain failing
        clientchain.return_err = true;
        assert!(
            verify_challenge(&dummy_hash, &clientchain, time::Duration::from_millis(10)).is_err(),
            "verify_challenge failed"
        );
    }

    #[test]
    fn get_challenge_response_test() {
        setup_logger();
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

        // then test with dummy hash but little time to fetch
        let mut dummy_response_set = ChallengeResponseIds::new();
        let _ = dummy_response_set.insert(dummy_bid.txid);
        vtx.send(ChallengeResponse(dummy_hash, dummy_bid.clone())).unwrap();
        let res = get_challenge_response(&dummy_hash, &vrx, time::Duration::from_nanos(1)).unwrap();
        assert_eq!(res.len(), 0);

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
    fn update_challenge_request_state_test() {
        setup_logger();
        let clientchain = MockClientChain::new();
        let service = MockService::new();
        let storage = Arc::new(MockStorage::new());

        let dummy_hash = gen_dummy_hash(11);
        let mut challenge = gen_challenge_state(&dummy_hash);
        let num_service_chain_blocks = challenge.request.end_blockheight - challenge.request.start_blockheight;

        // Test challenge state request set and stored correctly
        let _ = clientchain.height.replace(1);
        let mut comparison_challenge_request = challenge.request.clone(); // Clone request for comparison
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 1, 1);
        // All fields stay the same but start and end blockheight_clientchain
        comparison_challenge_request.start_blockheight_clientchain = *clientchain.height.borrow();
        comparison_challenge_request.end_blockheight_clientchain =
            challenge.request.start_blockheight_clientchain + num_service_chain_blocks; // start_height + number of servcie chain blocks
        assert_eq!(challenge.request, comparison_challenge_request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            comparison_challenge_request
        );
        // Test updating request with no diff
        let _ = clientchain
            .height
            .replace(challenge.request.start_blockheight_clientchain + 1);
        let _ = service.height.replace(challenge.request.start_blockheight as u64 + 1);
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 1, 1);
        // All fields should stay the same
        assert_eq!(challenge.request, comparison_challenge_request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            comparison_challenge_request
        );
        // Test updating request with faster service
        let _ = clientchain
            .height
            .replace(challenge.request.start_blockheight_clientchain + 1);
        let _ = service.height.replace(challenge.request.start_blockheight as u64 + 2);
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 1, 1);
        // All fields except clientchain end height that should be decreased
        comparison_challenge_request.end_blockheight_clientchain -= 1;
        assert_eq!(challenge.request, comparison_challenge_request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            comparison_challenge_request
        );
        // Test updating request with faster clientchain
        let _ = clientchain
            .height
            .replace(challenge.request.start_blockheight_clientchain + 2);
        let _ = service.height.replace(challenge.request.start_blockheight as u64 + 1);
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 1, 1);
        // All fields except clientchain end height that should be decreased
        comparison_challenge_request.end_blockheight_clientchain += 2; // 1 from before and 1 now
        assert_eq!(challenge.request, comparison_challenge_request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            comparison_challenge_request
        );

        // Test challenge state set and storage performed correctly
        // for client chain block time half of service chain block time
        let storage = Arc::new(MockStorage::new()); //reset storage
        let mut challenge = gen_challenge_state(&dummy_hash); // reset challenge
        let _ = clientchain.height.replace(1);
        let _ = service.height.replace(challenge.request.start_blockheight as u64);
        let mut comparison_challenge_request = challenge.request.clone(); // Clone request for comparison
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 2, 1);
        // All fields stay the same but start and end blockheight_clientchain
        comparison_challenge_request.start_blockheight_clientchain = *clientchain.height.borrow();
        comparison_challenge_request.end_blockheight_clientchain =
            challenge.request.start_blockheight_clientchain + 2 * num_service_chain_blocks; // start_height + (2 times client chain blocks as service chain blocks in same
                                                                                            // time period * number of service chain block)
        assert_eq!(challenge.request, comparison_challenge_request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            comparison_challenge_request
        );
        // Test updating request with no diff
        let _ = clientchain
            .height
            .replace(challenge.request.start_blockheight_clientchain + 2);
        let _ = service.height.replace(challenge.request.start_blockheight as u64 + 1);
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 2, 1);
        // All fields should stay the same
        assert_eq!(challenge.request, comparison_challenge_request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            comparison_challenge_request
        );
        // Test updating request with faster service
        let _ = clientchain
            .height
            .replace(challenge.request.start_blockheight_clientchain + 1);
        let _ = service.height.replace(challenge.request.start_blockheight as u64 + 2);
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 2, 1);
        // All fields except clientchain end height that should be decreased
        comparison_challenge_request.end_blockheight_clientchain -= 3;
        assert_eq!(challenge.request, comparison_challenge_request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            comparison_challenge_request
        );
        // Test updating request with faster clientchain
        let _ = clientchain
            .height
            .replace(challenge.request.start_blockheight_clientchain + 4);
        let _ = service.height.replace(challenge.request.start_blockheight as u64 + 1);
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 2, 1);
        // All fields except clientchain end height that should be decreased
        comparison_challenge_request.end_blockheight_clientchain += 5; // 3 from before and 2 now
        assert_eq!(challenge.request, comparison_challenge_request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            comparison_challenge_request
        );

        // Test stored version unchanged if attempt is made to store request a second
        // time
        let old_challenge = challenge.clone(); // save old challenge state
        challenge.request.fee_percentage = 25; // alter random field
        let new_challenge = challenge.clone(); // save new challenge state
        let _ = update_challenge_request_state(&clientchain, &service, storage.clone(), &mut challenge, 2, 1);
        assert_eq!(challenge.request, old_challenge.request);
        assert_eq!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            old_challenge.request
        );
        assert_ne!(challenge.request, new_challenge.request);
        assert_ne!(
            storage.get_request(challenge.request.txid).unwrap().unwrap(),
            new_challenge.request
        );
    }

    #[test]
    fn check_request_test() {
        setup_logger();
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
        setup_logger();
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
        setup_logger();
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
        setup_logger();
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
        storage
            .save_challenge_request_state(&challenge_state.request, &challenge_state.bids)
            .unwrap();

        let (vtx, vrx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

        let _ = clientchain.height.replace((dummy_request.start_blockheight) + 1); // set height +1 for challenge hash response
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
            Arc::new(RwLock::new(Some(challenge_state.clone()))),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            50,
            time::Duration::from_millis(10),
        );

        match res {
            Ok(_) => {
                let resps = storage.get_response(dummy_request.txid).unwrap();
                assert_eq!(resps, None);
                let bids = storage.get_bids(dummy_request.txid).unwrap();
                assert_eq!(challenge_state.bids, HashSet::from_iter(bids.iter().cloned()));
                let requests = storage.get_requests(None, None, None).unwrap();
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
            Arc::new(RwLock::new(Some(challenge_state.clone()))),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
            time::Duration::from_millis(10),
        );

        match res {
            Ok(_) => {
                let resps = storage.get_response(dummy_request.txid).unwrap();
                assert_eq!(
                    resps.unwrap(),
                    Response {
                        num_challenges: 4,
                        bid_responses: [(dummy_bid.txid, 1)].iter().cloned().collect()
                    }
                );
                assert_eq!(1, storage.challenge_responses.borrow().len());
                let bids = storage.get_bids(dummy_request.txid).unwrap();
                assert_eq!(challenge_state.bids, HashSet::from_iter(bids.iter().cloned()));
                let requests = storage.get_requests(None, None, None).unwrap();
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
            Arc::new(RwLock::new(Some(challenge_state))),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
            time::Duration::from_millis(10),
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
            Arc::new(RwLock::new(Some(challenge_state))),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
            time::Duration::from_millis(10),
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
            Arc::new(RwLock::new(Some(challenge_state))),
            &vrx,
            Arc::new(storage_err),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
            time::Duration::from_millis(10),
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
            Arc::new(RwLock::new(Some(challenge_state))),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
            time::Duration::from_millis(10),
        );
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(Error::Coordinator(e)) => {
                assert_eq!(0, storage.challenge_responses.borrow().len());
                assert_eq!(CError::UnverifiedChallenge.to_string(), e.to_string());
            }
            Err(_) => assert!(false, "should not return any error"),
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
            Arc::new(RwLock::new(Some(challenge_state))),
            &vrx,
            storage.clone(),
            time::Duration::from_millis(10),
            time::Duration::from_millis(10),
            1,
            time::Duration::from_millis(10),
        );
        match res {
            Ok(_) => {
                assert_eq!(0, storage.challenge_responses.borrow().len());
            }
            Err(_) => assert!(false, "should not return error"),
        }
    }
}
