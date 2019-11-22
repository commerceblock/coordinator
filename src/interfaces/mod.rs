//! # Interfaces
//!
//! Interfaces used by the coordinator library

pub mod bid;
pub mod clientchain;
pub mod request;
pub mod response;
pub mod service;
pub mod storage;

#[cfg(test)]
pub mod mocks;
