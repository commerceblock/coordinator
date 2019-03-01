//! # Main
//!
//! Main daemon entry

#[macro_use]
extern crate log;
extern crate coordinator;
extern crate env_logger;

use std::process;

fn main() {
    // To see results set RUST_LOG to one of the following:
    // info, warning, debug, error, coordinator(for all)
    env_logger::init();
    if let Err(e) = coordinator::daemon::run() {
        error!("daemon failure: {}", e);
        process::exit(1);
    }
}
