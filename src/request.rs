//! # Request
//!
//! Service request models for client requests and bids

use std::collections::HashSet;

use ocean_rpc::json::{GetRequestBidsResultBid, GetRequestsResult};

use bitcoin_hashes::sha256d;
use secp256k1::key::PublicKey;

/// Request struct storing info on client request and modelling data that need
/// to be stored
#[derive(Debug, PartialEq, Clone)]
pub struct Request {
    /// Ocean transaction ID of the request transaction
    pub txid: sha256d::Hash,
    /// Request start block height
    pub start_blockheight: usize,
    /// Request end block height
    pub end_blockheight: usize,
    /// Genesis blockhash of client issuing request
    pub genesis_blockhash: sha256d::Hash,
    /// Fee percentage for Guardnodes set by client
    pub fee_percentage: u32,
    /// Num of Guardnode tickets set by client
    pub num_tickets: u32,
}

impl Request {
    /// Return an instance of Request from an ocean json rpc GetRequestsResult
    pub fn from_json(res: &GetRequestsResult) -> Self {
        Request {
            txid: res.txid,
            start_blockheight: res.start_block_height as usize,
            end_blockheight: res.end_block_height as usize,
            genesis_blockhash: res.genesis_block,
            fee_percentage: res.fee_percentage,
            num_tickets: res.num_tickets,
        }
    }
}

/// Bid struct storing successful bids and modelling data that need to be stored
#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub struct Bid {
    /// Ocean transaction ID of the bid transaction
    pub txid: sha256d::Hash,
    /// Bid owner verification public key
    pub pubkey: PublicKey,
}

impl Bid {
    /// Return an instance of Bid from an ocean json rpc GetRequestBidsResultBid
    pub fn from_json(res: &GetRequestBidsResultBid) -> Self {
        Bid {
            txid: res.txid,
            pubkey: res.fee_pub_key,
        }
    }
}
/// Type defining a set of Bids
pub type BidSet = HashSet<Bid>;
