//! # Request
//!
//! Service request models for client requests and bids

use bitcoin::util::hash::Sha256dHash;
use secp256k1::key::PublicKey;

/// Request struct storing info on client request and modelling data that need
/// to be stored
#[derive(Debug, PartialEq)]
pub struct Request {
    /// Request start block height
    pub start_blockheight: usize,
    /// Request end block height
    pub end_blockheight: usize,
    /// Genesis blockhash of client issuing request
    pub genesis_blockhash: Sha256dHash,
    /// Fee percentage for Guardnodes set by client
    pub fee_percentage: u32,
    /// Num of Guardnode tickets set by client
    pub num_tickets: u32,
}

// TODO
// from json:RequestResult implementation
// impl Request {
//     pub fn from_json(getrequestresult) -> Self {

//     }
// }

/// Bid struct storing successful bids and modelling data that need to be stored
#[derive(Clone, Debug, PartialEq)]
pub struct Bid {
    /// Ocean transaction ID of the bid transaction
    pub txid: Sha256dHash,
    /// Bid owner verification public key
    pub pubkey: PublicKey,
}

// TODO
// from json:RequestResult implementation
// impl Bid {
//     pub fn from_json(getrequestbidsresult) -> Self {

//     }
// }
