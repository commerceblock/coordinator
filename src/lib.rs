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

extern crate ocean_rpc;
extern crate rust_ocean;

pub mod daemon;
