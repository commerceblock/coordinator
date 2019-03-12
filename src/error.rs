//! # Error
//!
//! Custom Error types for our crate

use std::error;
use std::fmt;
use std::result;

use bitcoin::util::hash::HexError;
use bitcoin_hashes::Error as HashesError;
use ocean_rpc::Error as OceanRpcError;
use secp256k1::Error as Secp256k1Error;

/// Crate specific Result for crate specific Errors
pub type Result<T> = result::Result<T, CError>;

/// The error type for errors produced in this crate.
#[derive(Debug)]
pub enum CError {
    /// Inherit all errors from ocean rpc
    OceanRpc(OceanRpcError),
    /// Bitcoin util hash hex error
    BitcoinHexError(HexError),
    /// Bitcoin hashes error
    BitcoinHashesError(HashesError),
    /// Secp256k1 error
    Secp256k1Error(Secp256k1Error),
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

impl From<HexError> for CError {
    fn from(e: HexError) -> CError {
        CError::BitcoinHexError(e)
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

impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CError::OceanRpc(ref e) => write!(f, "ocean rpc error: {}", e),
            CError::BitcoinHexError(ref e) => write!(f, "bitcoin hex error: {}", e),
            CError::BitcoinHashesError(ref e) => write!(f, "bitcoin hashes error: {}", e),
            CError::Secp256k1Error(ref e) => write!(f, "secp256k1 error: {}", e),
            CError::Coordinator(ref e) => write!(f, "service error: {}", e),
        }
    }
}

impl error::Error for CError {
    fn description(&self) -> &str {
        match *self {
            CError::OceanRpc(_) => "ocean rpc error",
            CError::BitcoinHexError(_) => "bitcoin hex error",
            CError::BitcoinHashesError(_) => "bitcoin hashes error",
            CError::Secp256k1Error(_) => "secp256k1 error",
            CError::Coordinator(_) => "service error",
        }
    }

    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            CError::OceanRpc(ref e) => Some(e),
            CError::BitcoinHexError(ref e) => Some(e),
            CError::BitcoinHashesError(ref e) => Some(e),
            CError::Secp256k1Error(ref e) => Some(e),
            CError::Coordinator(_) => None,
        }
    }
}
