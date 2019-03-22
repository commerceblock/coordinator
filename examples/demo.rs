//! Simple demo of coordinator with mock guard node

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
    rt::{self, Future},
    Body, Client, Method, Request,
};
use ocean_rpc::RpcApi;
use secp256k1::{Message, Secp256k1, SecretKey};

use coordinator::ocean::RpcClient;

/// Demo coordinator with listener and challenge service running
/// mock implementation for service chain interface and ocean
/// based client chain with auto block generation and mock
/// guardnode challenge response generation
fn main() {
    let mut config = coordinator::config::Config::new().unwrap();
    config.challenge_duration = 5;
    config.verify_duration = 10;

    env::set_var("RUST_LOG", &config.log_level);
    env_logger::init();

    let client_rpc = Arc::new(
        RpcClient::new(
            config.clientchain.host.clone(),
            Some(config.clientchain.user.clone()),
            Some(config.clientchain.pass.clone()),
        )
        .unwrap(),
    );

    // auto client chain block generation
    let client_rpc_clone = client_rpc.clone();
    thread::spawn(move || loop {
        thread::sleep(time::Duration::from_secs(5));
        if let Err(e) = client_rpc_clone.client.generate(1) {
            error!("{}", e);
        }
    });

    // guard node
    let asset_hash = config.clientchain.asset_hash.clone();
    let listener_host = config.listener_host.clone();
    thread::spawn(move || {
        guardnode(
            &client_rpc.clone(),
            sha256d::Hash::from_hex(&asset_hash).unwrap(),
            listener_host,
        );
    });

    // run coordinator
    if let Err(e) = coordinator::coordinator::run(Arc::new(config)) {
        error!("{}", e);
    }
}

/// Mock guardnode implementation parsing each new block and
/// searching for a transaction that includes the challenge asset
/// The hash of the tx found is signed and send to the coordinator
/// Bid info (key/txid) are based on MockService data for demo purpose
fn guardnode(client_rpc: &RpcClient, asset_hash: sha256d::Hash, listener_host: String) {
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[0xaa; 32]).expect("32 bytes within curve order");
    let mut prev_block_count = 0;
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
                            let sig = secp.sign(&msg, &secret_key);
                            let data = format!(
                                r#"
                            {{
                                "txid": "1234567890000000000000000000000000000000000000000000000000000000",
                                "pubkey": "026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3",
                                "hash": "{}",
                                "sig": "{}"
                            }}"#,
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
                                .map(|res| {
                                    println!("VOILA\n{}", res.status());
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
