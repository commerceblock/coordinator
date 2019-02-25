//! # Main
//!
//! Main daemon entry
//!

extern crate coordinator;

fn main() {
    coordinator::daemon::run();
}
