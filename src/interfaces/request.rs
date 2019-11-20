//! # Request
//!
//! Service request models for client requests

use bitcoin::hashes::sha256d;

use ocean_rpc::json::GetRequestsResult;
use serde::Serialize;

/// Request struct storing info on client request and modelling data that need
/// to be stored
#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct Request {
    /// Ocean transaction ID of the request transaction
    pub txid: sha256d::Hash,
    /// Request start block height
    pub start_blockheight: u32,
    /// Request end block height
    pub end_blockheight: u32,
    /// Genesis blockhash of client issuing request
    pub genesis_blockhash: sha256d::Hash,
    /// Fee percentage for Guardnodes set by client
    pub fee_percentage: u32,
    /// Num of Guardnode tickets set by client
    pub num_tickets: u32,
    /// Request client chain start block height
    pub start_blockheight_clientchain: u32,
    /// Request client chain end block height
    pub end_blockheight_clientchain: u32,
}

impl Request {
    /// Return an instance of Request from an ocean json rpc GetRequestsResult
    pub fn from_json(res: &GetRequestsResult) -> Self {
        Request {
            txid: res.txid,
            start_blockheight: res.start_block_height,
            end_blockheight: res.end_block_height,
            genesis_blockhash: res.genesis_block,
            fee_percentage: res.fee_percentage,
            num_tickets: res.num_tickets,
            start_blockheight_clientchain: 0,
            end_blockheight_clientchain: 0,
        }
    }
}
