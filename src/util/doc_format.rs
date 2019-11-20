//! doc_format
//!
//! doc format is used to store items in the db.
//! File contains methods to convert to/from document format.

use std::collections::HashMap;
use std::str::FromStr;

use bitcoin::hashes::{hex::FromHex, sha256d};
use bitcoin::secp256k1::PublicKey;
use bitcoin::Amount;
use mongodb::{ordered::OrderedDocument, Bson};
use ocean::Address;

use crate::interfaces::response::Response;
use crate::interfaces::{
    bid::{Bid, BidPayment},
    request::Request,
};

/// Util method that generates a Request document from a request
pub fn request_to_doc(request: &Request) -> OrderedDocument {
    doc! {
        "txid": request.txid.to_string(),
        "start_blockheight": request.start_blockheight,
        "end_blockheight": request.end_blockheight,
        "genesis_blockhash": request.genesis_blockhash.to_string(),
        "fee_percentage": request.fee_percentage,
        "num_tickets": request.num_tickets,
        "start_blockheight_clientchain": request.start_blockheight_clientchain,
        "end_blockheight_clientchain": request.end_blockheight_clientchain,
        "is_payment_complete": request.is_payment_complete,
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
        start_blockheight_clientchain: doc.get("start_blockheight_clientchain").unwrap().as_i32().unwrap() as u32,
        end_blockheight_clientchain: doc.get("end_blockheight_clientchain").unwrap().as_i32().unwrap() as u32,
        is_payment_complete: doc.get("is_payment_complete").unwrap().as_bool().unwrap(),
    }
}

/// Util method that generates a Bid document from a request bid
pub fn bid_to_doc(request_id: &Bson, bid: &Bid) -> OrderedDocument {
    let mut bid_doc = doc! {
        "request_id": request_id.clone(),
        "txid": bid.txid.to_string(),
        "pubkey": bid.pubkey.to_string(),
    };
    if let Some(payment) = &bid.payment {
        let mut bid_payment_doc = doc! {
            "address": payment.address.to_string(),
            "amount": payment.amount.as_btc(),
        };
        if let Some(txid) = payment.txid {
            let _ = bid_payment_doc.insert("txid", txid.to_string());
        }
        let _ = bid_doc.insert("payment", bid_payment_doc);
    }
    bid_doc
}

/// Util method that generates a request bid from a Bid document
pub fn doc_to_bid(doc: &OrderedDocument) -> Bid {
    let mut payment: Option<BidPayment> = None;
    if let Some(doc_payment) = doc.get("payment") {
        let doc_doc_payment = doc_payment.as_document().unwrap();
        let mut payment_txid: Option<sha256d::Hash> = None;
        if let Some(doc_payment_txid) = doc_doc_payment.get("txid") {
            payment_txid = Some(sha256d::Hash::from_hex(doc_payment_txid.as_str().unwrap()).unwrap())
        }
        payment = Some(BidPayment {
            txid: payment_txid,
            address: Address::from_str(doc_doc_payment.get("address").unwrap().as_str().unwrap()).unwrap(),
            amount: Amount::from_btc(doc_doc_payment.get("amount").unwrap().as_f64().unwrap()).unwrap(),
        });
    }
    Bid {
        txid: sha256d::Hash::from_hex(doc.get("txid").unwrap().as_str().unwrap()).unwrap(),
        pubkey: PublicKey::from_str(doc.get("pubkey").unwrap().as_str().unwrap()).unwrap(),
        payment: payment,
    }
}

/// Util method that generates a Response document from request response
pub fn response_to_doc(request_id: &Bson, response: &Response) -> OrderedDocument {
    let bid_resps_doc: OrderedDocument = response
        .bid_responses
        .iter()
        .map(|(key, val)| (key.to_string(), Bson::I32(*val as i32)))
        .collect();
    doc! {
        "request_id": request_id.clone(),
        "num_challenges": response.num_challenges,
        "bid_responses": bid_resps_doc
    }
}

/// Util method that generates request response from a Response document
pub fn doc_to_response(doc: &OrderedDocument) -> Response {
    let bid_resps: HashMap<sha256d::Hash, u32> = doc
        .get("bid_responses")
        .unwrap()
        .as_document()
        .unwrap()
        .iter()
        .map(|(key, val)| {
            (
                sha256d::Hash::from_hex(key.as_str()).unwrap(),
                val.as_i32().unwrap() as u32,
            )
        })
        .collect();
    Response {
        num_challenges: doc.get("num_challenges").unwrap().as_i32().unwrap() as u32,
        bid_responses: bid_resps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mongodb::oid::ObjectId;

    use crate::challenger::ChallengeResponseIds;
    use crate::util::testing::gen_dummy_hash;

    #[test]
    fn request_doc_test() {
        let request_hash = gen_dummy_hash(9);
        let genesis_hash = "1100000000000000000000000000000000000000000000000000000000000022";
        let request = Request {
            txid: request_hash,
            start_blockheight: 2,
            end_blockheight: 5,
            genesis_blockhash: sha256d::Hash::from_hex(genesis_hash).unwrap(),
            fee_percentage: 5,
            num_tickets: 10,
            start_blockheight_clientchain: 0,
            end_blockheight_clientchain: 0,
            is_payment_complete: false,
        };

        let doc = request_to_doc(&request);
        assert_eq!(
            doc! {
                "txid": request_hash.to_string(),
                "start_blockheight": 2,
                "end_blockheight": 5,
                "genesis_blockhash": genesis_hash,
                "fee_percentage": 5,
                "num_tickets": 10,
                "start_blockheight_clientchain":0,
                "end_blockheight_clientchain":0,
                "is_payment_complete": false,
            },
            doc
        );
        assert_eq!(request, doc_to_request(&doc));
    }

    #[test]
    fn bid_doc_test() {
        let id = ObjectId::new().unwrap();
        let pubkey_hex = "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3";
        let hash = gen_dummy_hash(1);
        let mut bid = Bid {
            txid: hash,
            pubkey: PublicKey::from_str(pubkey_hex).unwrap(),
            payment: None,
        };

        let doc = bid_to_doc(&Bson::ObjectId(id.clone()), &bid);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "txid": hash.to_string(),
                "pubkey": pubkey_hex
            },
            doc
        );
        assert_eq!(bid, doc_to_bid(&doc));

        let addr = "1HXfr2qBwT4qGZYn8FczNy68rw5dwG8trc";
        let amount = 56.123;
        let mut bid_payment = BidPayment {
            txid: None,
            address: Address::from_str(addr).unwrap(),
            amount: Amount::from_btc(amount).unwrap(),
        };
        bid.payment = Some(bid_payment.clone());
        let doc = bid_to_doc(&Bson::ObjectId(id.clone()), &bid);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "txid": hash.to_string(),
                "pubkey": pubkey_hex,
                "payment": doc!{
                    "address": addr,
                    "amount": amount
                }
            },
            doc
        );
        assert_eq!(bid, doc_to_bid(&doc));

        let payment_txid = gen_dummy_hash(123);
        bid_payment.txid = Some(payment_txid);
        bid.payment = Some(bid_payment.clone());
        let doc = bid_to_doc(&Bson::ObjectId(id.clone()), &bid);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "txid": hash.to_string(),
                "pubkey": pubkey_hex,
                "payment": doc!{
                    "address": addr,
                    "amount": amount,
                    "txid": payment_txid.to_string()
                }
            },
            doc
        );
        assert_eq!(bid, doc_to_bid(&doc));
    }

    #[test]
    fn response_doc_test() {
        let id = ObjectId::new().unwrap();
        let mut ids = ChallengeResponseIds::new();
        let mut resp = Response::new();

        let doc = response_to_doc(&Bson::ObjectId(id.clone()), &resp);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "num_challenges": 0,
                "bid_responses": doc! {}
            },
            doc
        );
        assert_eq!(resp, doc_to_response(&doc));

        let hash0 = gen_dummy_hash(0);
        let _ = ids.insert(hash0);
        resp.update(&ids);
        let doc = response_to_doc(&Bson::ObjectId(id.clone()), &resp);
        assert_eq!(
            doc! {
                "request_id": id.clone(),
                "num_challenges": 1,
                "bid_responses": doc! { gen_dummy_hash(0).to_string(): 1 }
            },
            doc
        );
        assert_eq!(resp, doc_to_response(&doc));

        let _ = ids.insert(gen_dummy_hash(1));
        let _ = ids.insert(gen_dummy_hash(2));
        let _ = ids.insert(gen_dummy_hash(3));
        resp.update(&ids);
        let doc = response_to_doc(&Bson::ObjectId(id.clone()), &resp);
        assert_eq!(&id, doc.get("request_id").unwrap().as_object_id().unwrap());
        assert_eq!(2, doc.get("num_challenges").unwrap().as_i32().unwrap());
        for (key, val) in doc.get_document("bid_responses").unwrap().iter() {
            if sha256d::Hash::from_hex(key.as_str()).unwrap() == hash0 {
                assert_eq!(2, val.as_i32().unwrap());
            } else {
                assert_eq!(1, val.as_i32().unwrap());
            }
            assert!(ids.contains(&sha256d::Hash::from_hex(key.as_str()).unwrap()));
        }
        assert_eq!(4, doc.get_document("bid_responses").unwrap().len());
        assert_eq!(resp, doc_to_response(&doc));
    }
}
