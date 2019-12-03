//! # Bid
//!
//! Service request models for bids and bid payments

use std::collections::HashSet;

use bitcoin::{hashes::sha256d, secp256k1::PublicKey, Amount};
use ocean::Address;
use ocean_rpc::json::GetRequestBidsResultBid;
use serde::{Serialize, Serializer};

/// Bid struct storing successful bids and modelling data that need to be stored
#[derive(Clone, Debug, PartialEq, Hash, Eq, Serialize)]
pub struct Bid {
    /// Ocean transaction ID of the bid transaction
    pub txid: sha256d::Hash,
    /// Bid owner verification public key
    #[serde(serialize_with = "serialize_pubkey")]
    pub pubkey: PublicKey,
    /// Bid payment optional
    pub payment: Option<BidPayment>,
}

impl Bid {
    /// Return an instance of Bid from an ocean json rpc GetRequestBidsResultBid
    pub fn from_json(res: &GetRequestBidsResultBid) -> Self {
        Bid {
            txid: res.txid,
            pubkey: res.fee_pub_key.key,
            payment: None,
        }
    }
}

/// Bid payment struct holding information for fee payments received by bid
/// owners
#[derive(Clone, Debug, PartialEq, Hash, Eq, Serialize)]
pub struct BidPayment {
    /// Bid payment transaction id; optional as might not be set yet
    pub txid: Option<sha256d::Hash>,
    /// Additional bid payment transaction ids, for when tx is split
    pub extra_txids: Option<Vec<sha256d::Hash>>,
    /// Bid pay to address
    pub address: Address,
    /// Bid amount expected
    #[serde(with = "bitcoin::util::amount::serde::as_btc")]
    pub amount: Amount,
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

    use util::testing::setup_logger;

    #[test]
    fn serialize_pubkey_test() {
        setup_logger();
        let txid_hex = "1234567890000000000000000000000000000000000000000000000000000000";
        let pubkey_hex = "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3";
        let bid = Bid {
            txid: sha256d::Hash::from_hex(txid_hex).unwrap(),
            pubkey: PublicKey::from_str(pubkey_hex).unwrap(),
            payment: None,
        };

        let serialized = serde_json::to_string(&bid);
        assert_eq!(
            format!(r#"{{"txid":"{}","pubkey":"{}","payment":null}}"#, txid_hex, pubkey_hex),
            serialized.unwrap()
        );
    }
}
