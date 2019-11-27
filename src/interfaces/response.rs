//! # Response
//!
//! Response model for service challenge responses

use std::collections::{HashMap, HashSet};

use bitcoin::hashes::sha256d;
use serde::Serialize;

/// Response struct that models responses to service challenges
/// by keeping track of the total number of challengers and the
/// number of challenges that each bid owner responded to
#[derive(Debug, Serialize, PartialEq)]
pub struct Response {
    /// Total number of challenges
    pub num_challenges: u32,
    /// Number of responses per bid txid
    pub bid_responses: HashMap<sha256d::Hash, u32>,
}

impl Response {
    /// Create new Response instance
    pub fn new() -> Response {
        Response {
            num_challenges: 0,
            bid_responses: HashMap::new(),
        }
    }

    /// Update Response struct from challenge response ids
    pub fn update(&mut self, responses: &HashSet<sha256d::Hash>) {
        self.num_challenges += 1;
        for txid in responses.iter() {
            let bid_entry = self.bid_responses.entry(*txid).or_insert(0);
            *bid_entry += 1;
        }
    }
}
