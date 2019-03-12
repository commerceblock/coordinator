//! Mock Client
//!
//! Mock Example of client sending a POST request to server

extern crate hyper;

use hyper::header::HeaderValue;
use hyper::rt::{self, Future, Stream};
use hyper::Client;
use hyper::{Body, Method, Request};

fn main() {
    let client = Client::new();

    let data = r#"
    {
        "hash": "0404040404040404040404040404040404040404040404040404040404040404",
        "pubkey": "0325bf82856a8fdcc7a2c08a933343d2c6332c4c252974d6b09b6232ea40804626",
        "sig": "304402201742daea5ec3b7306b9164be862fc1659cc830032180b8b17beffe02645860d602201039eba402d22e630308e6af05da8dd4f05b51b7d672ca5fc9e3b0a57776365c"
    }"#;

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
                println!("{:?}", chunk);
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
