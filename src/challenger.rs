//! Challenger
//!
//! Methods and models for fetching, structuring and running challenge requests

use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use bitcoin::util::hash::{HexError, Sha256dHash};

use crate::clientchain::ClientChain;
use crate::error::{CError, Result};
use crate::request::{Bid, Request};
use crate::service::Service;

static NUM_VERIFY_ATTEMPTS: u8 = 5;

/// Attempts to verify that a challenge has been included in the client chain
/// Method tries a fixed number of attempts NUM_VERIFY_ATTEMPTS with variable
// delay time between these to allow easy configuration
pub fn verify_challenge<K: ClientChain>(
    hash: &Sha256dHash,
    clientchain: &K,
    attempt_delay: time::Duration,
) -> Result<bool> {
    info! {"verifying challenge hash: {}", hash}
    for i in 0..NUM_VERIFY_ATTEMPTS {
        if clientchain.verify_challenge(&hash)? {
            info! {"challenge verified"}
            return Ok(true);
        }
        warn! {"attempt {} failed", i+1}
        if i + 1 == NUM_VERIFY_ATTEMPTS {
            break;
        }
        info! {"sleeping for {}sec...", attempt_delay.as_secs()}
        thread::sleep(attempt_delay)
    }
    Ok(false)
}

/// Get responses to the challenge by reading data from the channel receiver
/// Channel is read for a configurable duration and then the method returns
/// all the responses that have been received
pub fn get_challenge_responses(
    verify_rx: &Receiver<ChallengeResponse>,
    get_duration: time::Duration,
) -> Result<Vec<ChallengeResponse>> {
    let mut responses = vec![];

    let (dur_tx, dur_rx) = channel();
    let _ = thread::spawn(move || {
        thread::sleep(get_duration);
        dur_tx.send("tick").unwrap();
    });

    let mut time_to_break = false;
    while time_to_break == false {
        match verify_rx.try_recv() {
            Ok(resp) => responses.push(resp),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(CError::Coordinator(
                    "Challenge response receiver disconnected",
                ))
            }
        }
        let _ = dur_rx.try_recv().map(|_| time_to_break = true);
    }

    Ok(responses)
}

/// Run challenge for a specific request on the client chain
/// ...
/// ...
pub fn run_challenge_request<K: ClientChain>(
    clientchain: &K,
    challenge_state: Arc<Mutex<ChallengeState>>,
    verify_rx: Receiver<ChallengeResponse>,
) -> Result<()> {
    info! {"Running challenge request: {:?}", challenge_state.lock().unwrap().request};
    loop {
        let challenge_height = clientchain.get_blockheight()?;
        info! {"client chain height: {}", challenge_height}
        if challenge_state.lock().unwrap().request.end_blockheight < challenge_height as usize {
            break;
        }

        // send challenge
        info! {"sending challenge..."}
        let challenge_hash = clientchain.send_challenge()?;
        challenge_state.lock().unwrap().latest_challenge = Some(challenge_hash);

        // verify challenge
        if !verify_challenge(&challenge_hash, clientchain, time::Duration::from_secs(1))? {
            continue;
        }

        // get challenge proofs
        info! {"responses : {:?}", get_challenge_responses(&verify_rx, time::Duration::from_secs(1))?}

        thread::sleep(time::Duration::from_secs(1))
    }
    info! {"Challenge request ended"}
    Ok(())
}

/// Tuple struct to store a verified challenge response
/// for a winning bid on a specific challenge hash
#[derive(Debug)]
pub struct ChallengeResponse(pub Sha256dHash, pub Bid);

/// Mainstains challenge state with information on
/// challenge requests and bids as well as the
/// latest challenge hash in the client chain
#[derive(Debug)]
pub struct ChallengeState {
    /// Service Request for issuing challenges
    pub request: Request,
    /// Request winning bids that respond to challenges
    pub bids: Vec<Bid>,
    /// Latest challenge txid hash in the client chain
    pub latest_challenge: Option<Sha256dHash>,
}

/// Check if request start height has been reached in order to initiate
/// challenging.
fn check_request(request: &Request, height: u64) -> bool {
    return if request.start_blockheight <= height as usize {
        true
    } else {
        false
    };
}

/// Attempt to fetch the winnings bids for a request in the service chain
fn get_request_bids<T: Service>(request: &Request, service: &T) -> Result<Vec<Bid>> {
    match service.get_request_bids(&request.genesis_blockhash)? {
        Some(bids) => return Ok(bids),
        _ => Err(CError::Coordinator("No bids found")),
    }
}

/// Fetch next challenge state given a request and bids in the service chain
/// A challenge is fetched only when a request exists and the required
/// starting blockheight has been reached in the corresponding client chain
pub fn fetch_next<T: Service, K: ClientChain>(
    service: &T,
    clientchain: &K,
    genesis: &Sha256dHash,
) -> Result<Option<ChallengeState>> {
    info!("Fetching challenge request!");
    match service.get_request(&genesis)? {
        Some(req) => {
            let height = clientchain.get_blockheight()?;
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
    use crate::clientchain::MockClientChain;
    use crate::service::MockService;
    use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

    #[test]
    fn verify_challenge_test() {
        let mut clientchain = MockClientChain::new();
        let dummy_hash = clientchain.send_challenge().unwrap();

        assert!(
            verify_challenge(&dummy_hash, &clientchain, time::Duration::from_nanos(1)).unwrap()
                == true
        );

        clientchain.return_false = true;
        assert!(
            verify_challenge(&dummy_hash, &clientchain, time::Duration::from_nanos(1)).unwrap()
                == false
        );
        clientchain.return_false = false;

        clientchain.return_err = true;
        assert!(
            verify_challenge(&dummy_hash, &clientchain, time::Duration::from_nanos(1)).is_err(),
            "verify_challenge failed"
        )
    }

    #[test]
    fn get_challenge_responses_test() {
        let service = MockService::new();
        let clientchain = MockClientChain::new();

        let dummy_hash = clientchain.send_challenge().unwrap();
        let dummy_bid = service.get_request_bids(&dummy_hash).unwrap().unwrap()[0].clone();

        let (vtx, vrx): (Sender<ChallengeResponse>, Receiver<ChallengeResponse>) = channel();

        // first test with empty response
        let res = get_challenge_responses(&vrx, time::Duration::from_millis(1));
        assert_eq!(res.unwrap().len(), 0);

        // then test with a few dummy responses
        vtx.send(ChallengeResponse(dummy_hash, dummy_bid.clone()))
            .unwrap();
        vtx.send(ChallengeResponse(dummy_hash, dummy_bid.clone()))
            .unwrap();
        vtx.send(ChallengeResponse(dummy_hash, dummy_bid.clone()))
            .unwrap();
        let res = get_challenge_responses(&vrx, time::Duration::from_millis(10)).unwrap();
        assert_eq!(res.len(), 3);
        assert_eq!(res[0].0, dummy_hash);
        assert_eq!(res[0].1, dummy_bid);
        assert_eq!(res[1].0, dummy_hash);
        assert_eq!(res[1].1, dummy_bid);
        assert_eq!(res[2].0, dummy_hash);
        assert_eq!(res[2].1, dummy_bid);

        // then drop channel sender and test correct error is returned
        std::mem::drop(vtx);
        let res = get_challenge_responses(&vrx, time::Duration::from_millis(1));
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(CError::Coordinator("Challenge response receiver disconnected")) => assert!(true),
            Err(_) => assert!(false, "should not return any error"),
        }
    }

    #[test]
    fn check_request_test() {
        let clientchain = MockClientChain::new();
        let dummy_hash = clientchain.send_challenge().unwrap();

        let service = MockService::new();
        let dummy_request = service.get_request(&dummy_hash).unwrap().unwrap();

        assert!(dummy_request.start_blockheight == 2);
        assert!(check_request(&dummy_request, 1) == false);
        assert!(check_request(&dummy_request, 3) == true);
        assert!(check_request(&dummy_request, 2) == true);
    }

    #[test]
    fn get_request_bids_test() {
        let clientchain = MockClientChain::new();
        let dummy_hash = clientchain.send_challenge().unwrap();

        let mut service = MockService::new();
        let dummy_request = service.get_request(&dummy_hash).unwrap().unwrap();
        let dummy_bid = service.get_request_bids(&dummy_hash).unwrap().unwrap()[0].clone();

        // first test with bids
        let res = get_request_bids(&dummy_request, &service).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0], dummy_bid);

        // then test with None result
        service.return_none = true;
        let res = get_request_bids(&dummy_request, &service);
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(CError::Coordinator("No bids found")) => assert!(true),
            Err(_) => assert!(false, "should not return any error"),
        }
        service.return_none = false;

        // then test with Err result
        service.return_err = true;
        let res = get_request_bids(&dummy_request, &service);
        match res {
            Ok(_) => assert!(false, "should not return Ok"),
            Err(CError::Coordinator("No bids found")) => {
                assert!(false, "should not specific error")
            }
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn fetch_next_test() {
        let mut clientchain = MockClientChain::new();
        let dummy_hash = clientchain.send_challenge().unwrap();

        let mut service = MockService::new();
        let dummy_request = service.get_request(&dummy_hash).unwrap().unwrap();
        let dummy_bid = service.get_request_bids(&dummy_hash).unwrap().unwrap()[0].clone();

        // first test what happens when clientchain fails
        clientchain.return_err = true;
        assert!(fetch_next(&service, &clientchain, &dummy_hash).is_err());
        clientchain.return_err = false;

        // then test when get_request returns none
        service.return_none = true;
        let res = fetch_next(&service, &clientchain, &dummy_hash).unwrap();
        match res {
            None => assert!(true),
            Some(_v) => assert!(false, "not expecting value"),
        }
        service.return_none = false;

        // then test when get_request returns Request
        clientchain.height = dummy_request.start_blockheight as u64;
        let res = fetch_next(&service, &clientchain, &dummy_hash)
            .unwrap()
            .unwrap();
        assert_eq!(res.latest_challenge, None);
        assert_eq!(res.bids.len(), 1);
        assert_eq!(res.bids[0], dummy_bid);
        assert_eq!(res.request, dummy_request);
    }
}
