//! # Main
//!
//! Main daemon entry

#[macro_use]
extern crate log;
extern crate coordinator;
extern crate env_logger;

use std::env;
use std::process;
use std::sync::Arc;

fn main() {
    // Fetch config which is set from default values in config
    // and any values overriden by the corresponding env variable
    let config = Arc::new(coordinator::config::Config::new().unwrap());

    // To see results set RUST_LOG to one of the following:
    // info, warning, debug, error, coordinator(for all)
    env::set_var("RUST_LOG", &config.log_level);
    // Init env logger with value set from config
    env_logger::init();

    if let Err(e) = coordinator::coordinator::run(config) {
        error!("daemon failure: {}", e);
        process::exit(1);
    }
}
