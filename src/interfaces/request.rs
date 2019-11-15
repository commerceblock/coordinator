//! # Request
//!
//! Service request models for client requests and bids

use std::collections::HashSet;

use bitcoin::hashes::sha256d;
use bitcoin::secp256k1::PublicKey;
use ocean_rpc::json::{GetRequestBidsResultBid, GetRequestsResult};
use serde::{Serialize, Serializer};

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

/// Bid struct storing successful bids and modelling data that need to be stored
#[derive(Clone, Debug, PartialEq, Hash, Eq, Serialize)]
pub struct Bid {
    /// Ocean transaction ID of the bid transaction
    pub txid: sha256d::Hash,
    /// Bid owner verification public key
    #[serde(serialize_with = "serialize_pubkey")]
    pub pubkey: PublicKey,
}

impl Bid {
    /// Return an instance of Bid from an ocean json rpc GetRequestBidsResultBid
    pub fn from_json(res: &GetRequestBidsResultBid) -> Self {
        Bid {
            txid: res.txid,
            pubkey: res.fee_pub_key.key,
        }
    }
}

/// Type defining a set of Bids
pub type BidSet = HashSet<Bid>;

/// Custom serializer for type PublicKey in order to serialize
/// the key into a string and not the default u8 vector
fn serialize_pubkey<S>(x: &PublicKey, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&x.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::str::FromStr;

    use bitcoin::hashes::hex::FromHex;

    #[test]
    fn serialize_pubkey_test() {
        let txid_hex = "1234567890000000000000000000000000000000000000000000000000000000";
        let pubkey_hex = "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3";
        let bid = Bid {
            txid: sha256d::Hash::from_hex(txid_hex).unwrap(),
            pubkey: PublicKey::from_str(pubkey_hex).unwrap(),
        };

        let serialized = serde_json::to_string(&bid);
        assert_eq!(
            format!(r#"{{"txid":"{}","pubkey":"{}"}}"#, txid_hex, pubkey_hex),
            serialized.unwrap()
        );
    }
}
