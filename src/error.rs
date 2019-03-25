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
pub type Result<T> = result::Result<T, Error>;

/// The error type for errors produced in this crate.
#[derive(Debug)]
pub enum Error {
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

impl From<OceanRpcError> for Error {
    fn from(e: OceanRpcError) -> Error {
        Error::OceanRpc(e)
    }
}

impl From<String> for Error {
    fn from(e: String) -> Error {
        Error::Coordinator(e)
    }
}

impl From<HashesError> for Error {
    fn from(e: HashesError) -> Error {
        Error::BitcoinHashes(e)
    }
}

impl From<Secp256k1Error> for Error {
    fn from(e: Secp256k1Error) -> Error {
        Error::Secp256k1(e)
    }
}

impl From<ConfigError> for Error {
    fn from(e: ConfigError) -> Error {
        Error::Config(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::OceanRpc(ref e) => write!(f, "ocean rpc error: {}", e),
            Error::BitcoinHashes(ref e) => write!(f, "bitcoin hashes error: {}", e),
            Error::Secp256k1(ref e) => write!(f, "secp256k1 error: {}", e),
            Error::Config(ref e) => write!(f, "config error: {}", e),
            Error::Coordinator(ref e) => write!(f, "service error: {}", e),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::OceanRpc(ref e) => Some(e),
            Error::BitcoinHashes(ref e) => Some(e),
            Error::Secp256k1(ref e) => Some(e),
            Error::Config(ref e) => Some(e),
            Error::Coordinator(_) => None,
        }
    }
}
