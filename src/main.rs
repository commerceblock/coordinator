//! # Main
//!
//! Main daemon entry

extern crate coordinator;

use std::process;

fn main() {
    if let Err(e) = coordinator::daemon::run() {
        println!("daemon failure: {}", e);
        process::exit(1);
    }
}
