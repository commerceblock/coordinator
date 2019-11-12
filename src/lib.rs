//! # Coordinator Library
//!
//! Core functionality of the coordinator library

// Coding conventions
#![deny(non_upper_case_globals)]
#![deny(non_camel_case_types)]
#![deny(non_snake_case)]
#![deny(unused_mut)]
#![deny(missing_docs)]
#![warn(unsafe_code)]
#![warn(unreachable_pub)]
#![warn(unused_extern_crates)]
#![warn(unused_import_braces)]
#![warn(unused_results)]
#![warn(unused_imports)] // alow this for now - remove later
#![allow(dead_code)] // alow this for now - remove later

#[macro_use]
extern crate log;
extern crate base64;
extern crate bitcoin;
extern crate bitcoin_hashes;
extern crate config as config_rs;
extern crate futures;
extern crate hyper;
extern crate ocean_rpc;
extern crate rust_ocean as _ocean;
extern crate secp256k1;
extern crate serde as serde;
extern crate serde_json;
#[macro_use]
extern crate mongodb;
extern crate jsonrpc_http_server;

pub mod api;
pub mod challenger;
pub mod clientchain;
pub mod config;
pub mod coordinator;
pub mod error;
pub mod listener;
pub mod request;
pub mod response;
pub mod service;
pub mod storage;
/// utilities
pub mod util;
