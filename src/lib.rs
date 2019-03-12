//! # Coordinator Library
//!
//! Basic functionality required by Coordinator daemon

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
#![allow(unused_imports)] // alow this for now - remove later
#![allow(dead_code)] // alow this for now - remove later

#[macro_use]
extern crate log;

extern crate bitcoin;
extern crate bitcoin_hashes;
extern crate futures;
extern crate hyper;
extern crate ocean_rpc;
extern crate rust_ocean as _ocean;
extern crate secp256k1;
extern crate serde as _serde;
extern crate serde_json;

pub mod challenger;
pub mod clientchain;
pub mod coordinator;
pub mod error;
pub mod listener;
pub mod request;
pub mod service;
pub mod storage;
