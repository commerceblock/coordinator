//! # Testing Utils
//!
//! Colleciton of helper functions used in tests module

use std::env;
use std::str::FromStr;
use std::sync::Once;

use bitcoin::hashes::{hex::FromHex, sha256d, Hash};
use bitcoin::secp256k1::PublicKey;

use crate::challenger::ChallengeState;
use crate::interfaces::{
    bid::{Bid, BidSet},
    request::Request as ServiceRequest,
};

static INIT: Once = Once::new();

/// Setup logger function that is only run once
pub fn setup_logger() {
    INIT.call_once(|| {
        env::set_var("RUST_LOG", "debug");
        env_logger::init();
    });
}

/// Generate dummy hash
pub fn gen_dummy_hash(i: u8) -> sha256d::Hash {
    sha256d::Hash::from_slice(&[i as u8; 32]).unwrap()
}

/// Generate dummy challenge state
pub fn gen_challenge_state(request_hash: &sha256d::Hash) -> ChallengeState {
    let request = ServiceRequest {
        txid: *request_hash,
        start_blockheight: 2,
        end_blockheight: 5,
        genesis_blockhash: gen_dummy_hash(0),
        fee_percentage: 5,
        num_tickets: 10,
        start_blockheight_clientchain: 0,
        end_blockheight_clientchain: 0,
        is_payment_complete: false,
    };
    let mut bids = BidSet::new();
    let _ = bids.insert(Bid {
        txid: sha256d::Hash::from_hex("1234567890000000000000000000000000000000000000000000000000000000").unwrap(),
        pubkey: PublicKey::from_str("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3").unwrap(),
        payment: None,
    });
    ChallengeState {
        request,
        bids,
        latest_challenge: Some(gen_dummy_hash(0)),
    }
}

/// Generate dummy challenge state with specific challenge
pub fn gen_challenge_state_with_challenge(
    request_hash: &sha256d::Hash,
    challenge_hash: &sha256d::Hash,
) -> ChallengeState {
    let request = ServiceRequest {
        txid: sha256d::Hash::from_slice(&[0xff as u8; 32]).unwrap(),
        start_blockheight: 2,
        end_blockheight: 5,
        genesis_blockhash: *request_hash,
        fee_percentage: 5,
        num_tickets: 10,
        start_blockheight_clientchain: 0,
        end_blockheight_clientchain: 0,
        is_payment_complete: false,
    };
    let mut bids = BidSet::new();
    let _ = bids.insert(Bid {
        txid: sha256d::Hash::from_hex("1234567890000000000000000000000000000000000000000000000000000000").unwrap(),
        // pubkey corresponding to SecretKey::from_slice(&[0xaa; 32])
        pubkey: PublicKey::from_str("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3").unwrap(),
        payment: None,
    });
    ChallengeState {
        request,
        bids,
        latest_challenge: Some(*challenge_hash),
    }
}
