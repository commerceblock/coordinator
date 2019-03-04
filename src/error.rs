//! # Error
//!
//! Custom Error types for our crate

use std::error;
use std::fmt;
use std::result;

use bitcoin;
use ocean_rpc::Error as OceanRpcError;

/// Crate specific Result for crate specific Errors
pub type Result<T> = result::Result<T, CError>;

/// The error type for errors produced in this crate.
#[derive(Debug)]
pub enum CError {
    /// Inherit all errors from ocean rpc
    OceanRpc(OceanRpcError),
    /// Coordinator error
    Coordinator(&'static str),
}

impl From<OceanRpcError> for CError {
    fn from(e: OceanRpcError) -> CError {
        CError::OceanRpc(e)
    }
}

impl From<&'static str> for CError {
    fn from(e: &'static str) -> CError {
        CError::Coordinator(e)
    }
}

impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CError::OceanRpc(ref e) => write!(f, "ocean rpc error: {}", e),
            CError::Coordinator(ref e) => write!(f, "service error: {}", e),
        }
    }
}

impl error::Error for CError {
    fn description(&self) -> &str {
        match *self {
            CError::OceanRpc(_) => "ocean rpc error",
            CError::Coordinator(_) => "service error",
        }
    }

    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            CError::OceanRpc(ref e) => Some(e),
            CError::Coordinator(_) => None,
        }
    }
}
