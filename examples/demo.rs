//! Simple demo of coordinator with mock guard node
//!
//! Demo Coordinator connects to demo client chain (can be built by running
//! ./scripts/demo.sh) and sends challenges to mock guardnodes that made bids.
//! When guardnode responses are received by coordinator they are verified and
//! can be stored ready for fee payments to be made.

#[macro_use]
extern crate log;
extern crate bitcoin;
extern crate bitcoin_hashes;
extern crate coordinator;
extern crate env_logger;
extern crate hyper;
extern crate ocean_rpc;
extern crate secp256k1;

use std::sync::Arc;
use std::{env, thread, time};

use bitcoin::consensus::encode::serialize;
use bitcoin_hashes::{hex::FromHex, hex::ToHex, sha256d};
use hyper::{
    rt::{self, Future, Stream},
    Body, Client, Method, Request,
};
use ocean_rpc::RpcApi;
use secp256k1::{Message, Secp256k1, SecretKey};

use coordinator::clientchain::get_first_unspent;
use coordinator::coordinator as coordinator_main;
use coordinator::ocean::OceanClient;
use coordinator::service::SERVICE_BLOCK_TIME;

/// Demo coordinator with listener and challenge service running
/// mock implementation for service chain interface and ocean
/// based client chain with auto block generation and mock
/// guardnode challenge response generation
fn main() {
    let mut config = coordinator::config::Config::new().unwrap();
    config.challenge_duration = 5;

    env::set_var("RUST_LOG", &config.log_level);
    env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

    let client_rpc = Arc::new(
        OceanClient::new(
            config.clientchain.host.clone(),
            Some(config.clientchain.user.clone()),
            Some(config.clientchain.pass.clone()),
        )
        .unwrap(),
    );

    // auto client chain block generation
    let client_rpc_clone = client_rpc.clone();
    thread::spawn(move || loop {
        thread::sleep(time::Duration::from_secs(SERVICE_BLOCK_TIME));
        if let Err(e) = client_rpc_clone.clone().client.generate(1) {
            error!("{}", e);
        }
    });

    let genesis_hash = sha256d::Hash::from_hex(&config.clientchain.genesis_hash).unwrap();
    let request = &client_rpc.get_requests(Some(&genesis_hash));
    if request.as_ref().unwrap().is_empty() {
        panic!("No active request in client blockchain!")
    }
    let request_txid = request.as_ref().unwrap()[0].txid;
    let guardnode_pubkey = "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3";
    let mut guardnode_txid = genesis_hash; // dummy init
    for bid in client_rpc.get_request_bids(&request_txid).unwrap().unwrap().bids {
        if bid.fee_pub_key.to_string() == guardnode_pubkey {
            guardnode_txid = bid.txid;
            println!("Guardnode bid txid: {}", guardnode_txid);
            break;
        }
    }

    // add two guardnodes with valid keys and one without
    // keys based on mockservice request bids
    let listener_host = config.listener_host.clone();
    let client_rpc_clone = client_rpc.clone();
    thread::spawn(move || {
        guardnode(
            &client_rpc_clone,
            listener_host.clone(),
            guardnode_txid,
            SecretKey::from_slice(&[0xaa; 32]).unwrap(),
            String::from("026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3"),
        );
    });

    coordinator_main::run(config).unwrap()
}

/// Mock guardnode implementation parsing each new block and
/// searching for a transaction that includes the challenge asset
/// The hash of the tx found is signed and send to the coordinator
/// Bid info (key/txid) are based on MockService data for demo purpose
fn guardnode(
    client_rpc: &OceanClient,
    listener_host: String,
    guard_txid: sha256d::Hash,
    guard_key: SecretKey,
    guard_pubkey: String,
) {
    let secp = Secp256k1::new();
    let mut prev_block_count = 0;
    // Get asset hash from unspent list
    let asset_hash;
    match get_first_unspent(&client_rpc, &String::from("CHALLENGE")) {
        Err(_) => panic!("No challenge asset issued in client blockchain!"),
        Ok(res) => asset_hash = res.asset,
    }

    loop {
        if let Ok(block_count) = client_rpc.get_block_count() {
            if block_count > prev_block_count {
                prev_block_count = block_count;
                let block = client_rpc
                    .get_block(&client_rpc.get_block_hash(block_count).unwrap())
                    .unwrap();

                for tx in block.txdata.iter() {
                    for out in tx.output.iter() {
                        if out.asset == rust_ocean::confidential::Asset::Explicit(asset_hash) {
                            let msg = Message::from_slice(&serialize(&tx.txid())).unwrap();
                            let sig = secp.sign(&msg, &guard_key);
                            let data = format!(
                                r#"
                            {{
                                "txid": "{}",
                                "pubkey": "{}",
                                "hash": "{}",
                                "sig": "{}"
                            }}"#,
                                guard_txid.to_string(),
                                guard_pubkey,
                                tx.txid(),
                                sig.serialize_der().to_hex()
                            );
                            let uri: hyper::Uri = format!("http://{}/challengeproof", listener_host).parse().unwrap();
                            let mut req = Request::new(Body::from(data));
                            *req.method_mut() = Method::POST;
                            *req.uri_mut() = uri.clone();
                            let client = Client::new();
                            let ep = client
                                .request(req)
                                .and_then(|res| {
                                    println!("VOILA\n{}", res.status());
                                    res.into_body().concat2().map(|chunk| {
                                        println!("body: {}", String::from_utf8_lossy(&chunk));
                                    })
                                })
                                .map_err(|err| {
                                    println!("{}", err);
                                });
                            drop(client);
                            rt::run(ep);
                        }
                    }
                }
            }
        }
        thread::sleep(time::Duration::from_millis(100));
    }
}
