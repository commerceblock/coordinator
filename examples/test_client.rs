//! Mock Client
//!
//! Mock Example of client sending a POST request to server

extern crate bitcoin;
extern crate bitcoin_hashes;
extern crate hyper;
extern crate secp256k1;

use bitcoin::consensus::encode::serialize;
use bitcoin::util::hash::Sha256dHash;
use bitcoin_hashes::hex::ToHex;
use hyper::{
    header::HeaderValue,
    rt::{self, Future, Stream},
    Body, Client, Method, Request,
};
use secp256k1::{Message, Secp256k1, SecretKey};

fn main() {
    let client = Client::new();

    let hash = Sha256dHash::from_hex("0404040404040404040404040404040404040404040404040404040404040404").unwrap();
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[0xaa; 32]).expect("32 bytes within curve order");

    let msg = Message::from_slice(&serialize(&hash)).unwrap();
    let sig = secp.sign(&msg, &secret_key);

    let data = format!(
        r#"
    {{
        "txid": "1234567890000000000000000000000000000000000000000000000000000000",
        "pubkey": "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3",
        "hash": "{}",
        "sig": "{}"
    }}"#,
        hash,
        sig.serialize_der().to_hex()
    );

    let uri: hyper::Uri = "http://localhost:9999/challengeproof".parse().unwrap();
    let mut req = Request::new(Body::from(data));
    *req.method_mut() = Method::POST;
    *req.uri_mut() = uri.clone();
    req.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );

    let ep = client
        .request(req)
        .and_then(|res| {
            println!("Response: {}", res.status());
            println!("Headers: {:#?}", res.headers());

            res.into_body().concat2().map(|chunk| {
                println!("body: {}", String::from_utf8_lossy(&chunk));
            })
        })
        .map(|_| {
            println!("Done.");
        })
        .map_err(|err| {
            eprintln!("Error: {}", err);
        });

    drop(client);
    rt::run(ep);
}
