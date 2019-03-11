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

    let json = r#"{"hash":"0606060606060606060606060606060606060606060606060606060606060606"}"#;
    let uri: hyper::Uri = "http://localhost:9999/challengeproof".parse().unwrap();
    let mut req = Request::new(Body::from(json));
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
