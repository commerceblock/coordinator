//! Coordinator
//!
//! Coordinator entry point for spawning all components

use bitcoin::util::hash::{HexError, Sha256dHash};
use clientchain::MockClientChain;

use crate::error::{CError, Result};
use crate::service::{MockService, Service};

/// Run method
pub fn run() -> Result<()> {
    info!("Running coordinator!");

    let service = MockService {};
    let clientchain = MockClientChain {};

    ::challenger::run_challenger(service, clientchain)
}
