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
    BitcoinHashes(HashesError),
    /// Secp256k1 error
    Secp256k1(Secp256k1Error),
    /// Config error
    Config(ConfigError),
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
        CError::BitcoinHashes(e)
    }
}

impl From<Secp256k1Error> for CError {
    fn from(e: Secp256k1Error) -> CError {
        CError::Secp256k1(e)
    }
}

impl From<ConfigError> for CError {
    fn from(e: ConfigError) -> CError {
        CError::Config(e)
    }
}

impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CError::OceanRpc(ref e) => write!(f, "ocean rpc error: {}", e),
            CError::BitcoinHashes(ref e) => write!(f, "bitcoin hashes error: {}", e),
            CError::Secp256k1(ref e) => write!(f, "secp256k1 error: {}", e),
            CError::Config(ref e) => write!(f, "config error: {}", e),
            CError::Coordinator(ref e) => write!(f, "service error: {}", e),
        }
    }
}

impl error::Error for CError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            CError::OceanRpc(ref e) => Some(e),
            CError::BitcoinHashes(ref e) => Some(e),
            CError::Secp256k1(ref e) => Some(e),
            CError::Config(ref e) => Some(e),
            CError::Coordinator(_) => None,
        }
    }
}
