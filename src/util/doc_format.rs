//! doc_format
//!
//! Methods to convert to/from document format

use bitcoin_hashes::{hex::FromHex, sha256d};
use mongodb::{ordered::OrderedDocument, Bson};
use secp256k1::key::PublicKey;
use std::str::FromStr;

use crate::challenger::ChallengeResponseIds;
use crate::request::{Bid, Request};

/// Util method that generates a Request document from a request
pub fn request_to_doc(request: &Request) -> OrderedDocument {
    doc! {
        "txid": request.txid.to_string(),
        "start_blockheight_serv": request.start_blockheight,
        "end_blockheight_serv": request.end_blockheight,
        "genesis_blockhash": request.genesis_blockhash.to_string(),
        "fee_percentage": request.fee_percentage,
        "num_tickets": request.num_tickets
        // "start_blockheight_cli": request.start_blockheight,
        // "end_blockheight_cli": request.end_blockheight,
    }
}

/// Util method that generates a request from a Request document
pub fn doc_to_request(doc: &OrderedDocument) -> Request {
    Request {
        txid: sha256d::Hash::from_hex(doc.get("txid").unwrap().as_str().unwrap()).unwrap(),
        start_blockheight: doc.get("start_blockheight").unwrap().as_i32().unwrap() as u32,
        end_blockheight: doc.get("end_blockheight").unwrap().as_i32().unwrap() as u32,
        genesis_blockhash: sha256d::Hash::from_hex(doc.get("genesis_blockhash").unwrap().as_str().unwrap()).unwrap(),
        fee_percentage: doc.get("fee_percentage").unwrap().as_i32().unwrap() as u32,
        num_tickets: doc.get("num_tickets").unwrap().as_i32().unwrap() as u32,
    }
}

/// Util method that generates a Bid document from a request bid
pub fn bid_to_doc(request_id: &Bson, bid: &Bid) -> OrderedDocument {
    doc! {
        "request_id": request_id.clone(),
        "txid": bid.txid.to_string(),
        "pubkey": bid.pubkey.to_string()
    }
}

/// Util method that generates a request bid from a Bid document
pub fn doc_to_bid(doc: &OrderedDocument) -> Bid {
    Bid {
        txid: sha256d::Hash::from_hex(doc.get("txid").unwrap().as_str().unwrap()).unwrap(),
        pubkey: PublicKey::from_str(doc.get("pubkey").unwrap().as_str().unwrap()).unwrap(),
    }
}

/// Util method that generates a Response document from challenge responses
pub fn challenge_responses_to_doc(request_id: &Bson, responses: &ChallengeResponseIds) -> OrderedDocument {
    let bids = responses
        .iter()
        .map(|x| Bson::String(x.to_string()))
        .collect::<Vec<_>>();
    doc! {
        "request_id": request_id.clone(),
        "bid_txids": bids
    }
}

/// Util method that generates challenge responses from a Response document
pub fn doc_to_challenge_responses(doc: &OrderedDocument) -> ChallengeResponseIds {
    doc.get_array("bid_txids")
        .unwrap()
        .iter()
        .map(|x| sha256d::Hash::from_hex(x.as_str().unwrap()).unwrap())
        .collect()
}
