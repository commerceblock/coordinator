//! # Testing Utils
//!
//! Colleciton of helper functions used in tests module


use bitcoin_hashes::sha256d;

use bitcoin_hashes::{hex::FromHex, Hash};
use secp256k1::PublicKey;
use std::str::FromStr;

use crate::challenger::ChallengeState;
use crate::request::{Bid,BidSet, Request as ServiceRequest};

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
    };
    let mut bids = BidSet::new();
    let _ = bids.insert(Bid {
        txid: sha256d::Hash::from_hex("1234567890000000000000000000000000000000000000000000000000000000").unwrap(),
        pubkey: PublicKey::from_str("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3").unwrap(),
    });
    ChallengeState {
        request,
        bids,
        latest_challenge: Some(gen_dummy_hash(0)),
    }
}

/// Generate dummy challenge state with specific challenge
pub fn gen_challenge_state_with_challenge(request_hash: &sha256d::Hash, challenge_hash: &sha256d::Hash) -> ChallengeState {
    let request = ServiceRequest {
        txid: sha256d::Hash::from_slice(&[0xff as u8; 32]).unwrap(),
        start_blockheight: 2,
        end_blockheight: 5,
        genesis_blockhash: *request_hash,
        fee_percentage: 5,
        num_tickets: 10,
    };
    let mut bids = BidSet::new();
    let _ = bids.insert(Bid {
        txid: sha256d::Hash::from_hex("1234567890000000000000000000000000000000000000000000000000000000").unwrap(),
        // pubkey corresponding to SecretKey::from_slice(&[0xaa; 32])
        pubkey: PublicKey::from_str("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3").unwrap(),
    });
    ChallengeState {
        request,
        bids,
        latest_challenge: Some(*challenge_hash),
    }
}
