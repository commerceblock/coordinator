//! Storage
//!
//! Storage interface and implementations

use std::cell::RefCell;

use crate::challenger::{ChallengeResponse, ChallengeState};
use crate::error::{CError, Result};

/// Storage trait defining required functionality for objects that store request
/// and challenge information
pub trait Storage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: ChallengeState) -> Result<()>;
    /// Store responses to a specific challenge
    fn save_challenge_responses(&self, responses: Vec<ChallengeResponse>) -> Result<()>;
}

/// Database implementation of Storage trait
pub struct DbStorage {}

impl DbStorage {
    /// Create DbStorage instance
    pub fn new() -> Self {
        DbStorage {}
    }
}

//
// TODO: implement Storage trait for DbStorage
//

/// Mock implementation of Storage storing data in memory for testing
#[derive(Debug)]
pub struct MockStorage {
    /// Flag that when set returns error on all inherited methods that return
    /// Result
    pub return_err: bool,
    /// Store challenge states in memory
    pub challenge_states: RefCell<Vec<ChallengeState>>,
    /// Store challenge responses in memory
    pub challenge_responses: RefCell<Vec<ChallengeResponse>>,
}

impl MockStorage {
    /// Create a MockStorage with all flags turned off by default
    pub fn new() -> Self {
        MockStorage {
            return_err: false,
            challenge_states: RefCell::new(vec![]),
            challenge_responses: RefCell::new(vec![]),
        }
    }
}

impl Storage for MockStorage {
    /// Store the state of a challenge request
    fn save_challenge_state(&self, challenge: ChallengeState) -> Result<()> {
        if self.return_err {
            return Err(CError::Coordinator("save_challenge_state failed"));
        }
        info! {"{:?}", challenge}
        self.challenge_states.borrow_mut().push(challenge);
        Ok(())
    }

    /// Store responses to a specific challenge
    fn save_challenge_responses(&self, responses: Vec<ChallengeResponse>) -> Result<()> {
        if self.return_err {
            return Err(CError::Coordinator("save_challenge_responses failed"));
        }
        self.challenge_responses.borrow_mut().extend(responses);
        Ok(())
    }
}
