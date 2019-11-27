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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::util::testing::gen_dummy_hash;

    #[test]
    fn response_update() {
        let mut resp = Response::new();
        assert_eq!(0, resp.num_challenges);
        assert_eq!(0, resp.bid_responses.len());

        let hash_a = gen_dummy_hash(4);
        let hash_b = gen_dummy_hash(2);
        let hash_c = gen_dummy_hash(9);

        let mut txids = HashSet::new();
        resp.update(&txids);
        assert_eq!(1, resp.num_challenges);
        assert_eq!(0, resp.bid_responses.len());

        let _ = txids.insert(hash_a);
        let _ = txids.insert(hash_c);
        resp.update(&txids);
        assert_eq!(2, resp.num_challenges);
        assert_eq!(1, *resp.bid_responses.get(&hash_a).unwrap());
        assert_eq!(1, *resp.bid_responses.get(&hash_c).unwrap());

        txids.clear();
        let _ = txids.insert(hash_b);
        let _ = txids.insert(hash_c);
        resp.update(&txids);
        assert_eq!(3, resp.num_challenges);
        assert_eq!(1, *resp.bid_responses.get(&hash_a).unwrap());
        assert_eq!(1, *resp.bid_responses.get(&hash_b).unwrap());
        assert_eq!(2, *resp.bid_responses.get(&hash_c).unwrap());

        txids.clear();
        let _ = txids.insert(hash_a);
        let _ = txids.insert(hash_b);
        let _ = txids.insert(hash_c);
        resp.update(&txids);
        assert_eq!(4, resp.num_challenges);
        assert_eq!(2, *resp.bid_responses.get(&hash_a).unwrap());
        assert_eq!(2, *resp.bid_responses.get(&hash_b).unwrap());
        assert_eq!(3, *resp.bid_responses.get(&hash_c).unwrap());

        txids.clear();
        resp.update(&txids);
        assert_eq!(5, resp.num_challenges);
        assert_eq!(2, *resp.bid_responses.get(&hash_a).unwrap());
        assert_eq!(2, *resp.bid_responses.get(&hash_b).unwrap());
        assert_eq!(3, *resp.bid_responses.get(&hash_c).unwrap());
    }
}
