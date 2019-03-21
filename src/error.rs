//! # Error
//!
//! Custom Error types for our crate

use std::error;
use std::fmt;
use std::result;

use bitcoin_hashes::Error as HashesError;
use config_rs::ConfigError;
use ocean_rpc::Error as OceanRpcError;
use secp256k1::Error as Secp256k1Error;

/// Crate specific Result for crate specific Errors
pub type Result<T> = result::Result<T, CError>;

/// The error type for errors produced in this crate.
#[derive(Debug)]
pub enum CError {
    /// Inherit all errors from ocean rpc
    OceanRpc(OceanRpcError),
    /// Bitcoin hashes error
    BitcoinHashesError(HashesError),
    /// Secp256k1 error
    Secp256k1Error(Secp256k1Error),
    /// Config error
    ConfigError(ConfigError),
    /// Coordinator error
    Coordinator(String),
}

impl From<OceanRpcError> for CError {
    fn from(e: OceanRpcError) -> CError {
        CError::OceanRpc(e)
    }
}

impl From<String> for CError {
    fn from(e: String) -> CError {
        CError::Coordinator(e)
    }
}

impl From<HashesError> for CError {
    fn from(e: HashesError) -> CError {
        CError::BitcoinHashesError(e)
    }
}

impl From<Secp256k1Error> for CError {
    fn from(e: Secp256k1Error) -> CError {
        CError::Secp256k1Error(e)
    }
}

impl From<ConfigError> for CError {
    fn from(e: ConfigError) -> CError {
        CError::ConfigError(e)
    }
}

impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CError::OceanRpc(ref e) => write!(f, "ocean rpc error: {}", e),
            CError::BitcoinHashesError(ref e) => write!(f, "bitcoin hashes error: {}", e),
            CError::Secp256k1Error(ref e) => write!(f, "secp256k1 error: {}", e),
            CError::ConfigError(ref e) => write!(f, "config error: {}", e),
            CError::Coordinator(ref e) => write!(f, "service error: {}", e),
        }
    }
}

impl error::Error for CError {
    fn description(&self) -> &str {
        match *self {
            CError::OceanRpc(_) => "ocean rpc error",
            CError::BitcoinHashesError(_) => "bitcoin hashes error",
            CError::Secp256k1Error(_) => "secp256k1 error",
            CError::ConfigError(_) => "config error",
            CError::Coordinator(_) => "service error",
        }
    }

    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            CError::OceanRpc(ref e) => Some(e),
            CError::BitcoinHashesError(ref e) => Some(e),
            CError::Secp256k1Error(ref e) => Some(e),
            CError::ConfigError(ref e) => Some(e),
            CError::Coordinator(_) => None,
        }
    }
}
