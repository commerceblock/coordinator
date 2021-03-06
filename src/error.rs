//! # Error
//!
//! Custom Error types for our crate

use std::error;
use std::fmt;
use std::result;

use bitcoin::hashes::hex::Error as HashesHexError;
use bitcoin::hashes::Error as HashesError;
use bitcoin::secp256k1::Error as Secp256k1Error;
use config_rs::ConfigError;
use mongodb::Error as MongoDbError;
use ocean::AddressError;
use ocean_rpc::Error as OceanRpcError;

/// Crate specific Result for crate specific Errors
pub type Result<T> = result::Result<T, Error>;

/// Coordinator library specific errors
#[derive(Debug)]
pub enum CError {
    /// Missing bids for a specific request error
    MissingBids,
    /// Challenge was not successfully verified
    UnverifiedChallenge,
    /// Listener receiver disconnected error
    ReceiverDisconnected,
    /// Missing unspent for challenge asset. Takes parameters asset label and
    /// chain
    MissingUnspent(String, String),
    /// Config input error. Takes parameter input error type
    InputError(InputErrorType, String),
    /// Generic error from string error message
    Generic(String),
}

impl From<String> for CError {
    fn from(e: String) -> CError {
        CError::Generic(e)
    }
}

/// Input parameter error types
#[derive(Debug)]
pub enum InputErrorType {
    /// Invalid private key string
    PrivKey,
    /// Invalid genesis hash string
    GenHash,
    /// Missing input argument
    MissingArgument,
}

impl InputErrorType {
    fn as_str(&self) -> &'static str {
        match *self {
            InputErrorType::PrivKey => "Private key input - must be base58check string of length 52",
            InputErrorType::GenHash => "Chain genesis hash input must be hexadecimal string of length 64",
            InputErrorType::MissingArgument => "Argument missing",
        }
    }
}

impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CError::Generic(ref e) => write!(f, "generic Error: {}", e),
            CError::InputError(ref error, ref value) => write!(f, "Input Error: {} (value: {})", error.as_str(), value),
            CError::MissingUnspent(ref asset, ref chain) => {
                write!(f, "No unspent found for {} asset on {} chain", asset, chain)
            }
            _ => f.write_str(error::Error::description(self)),
        }
    }
}

impl error::Error for CError {
    fn description(&self) -> &str {
        match *self {
            CError::Generic(_) => "Generic error",
            CError::MissingBids => "No bids found",
            CError::UnverifiedChallenge => "Challenge not successfully verified",
            CError::ReceiverDisconnected => "Challenge response receiver disconnected",
            CError::MissingUnspent(_, _) => "No unspent found for asset",
            CError::InputError(_, _) => "Input parameter error",
        }
    }
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            _ => None,
        }
    }
}

/// The error type for errors produced in this crate.
#[derive(Debug)]
pub enum Error {
    /// Inherit all errors from ocean rpc
    OceanRpc(OceanRpcError),
    /// Bitcoin hashes error
    BitcoinHashes(HashesError),
    /// Bitcoin hex hashes error
    BitcoinHashesHex(HashesHexError),
    /// Ocean address error
    OceanAddress(AddressError),
    /// Secp256k1 error
    Secp256k1(Secp256k1Error),
    /// Mongodb error
    MongoDb(MongoDbError),
    /// Config error
    Config(ConfigError),
    /// Coordinator error
    Coordinator(CError),
}

impl From<OceanRpcError> for Error {
    fn from(e: OceanRpcError) -> Error {
        Error::OceanRpc(e)
    }
}

impl From<CError> for Error {
    fn from(e: CError) -> Error {
        Error::Coordinator(e)
    }
}

impl From<HashesError> for Error {
    fn from(e: HashesError) -> Error {
        Error::BitcoinHashes(e)
    }
}

impl From<HashesHexError> for Error {
    fn from(e: HashesHexError) -> Error {
        Error::BitcoinHashesHex(e)
    }
}

impl From<Secp256k1Error> for Error {
    fn from(e: Secp256k1Error) -> Error {
        Error::Secp256k1(e)
    }
}

impl From<AddressError> for Error {
    fn from(e: AddressError) -> Error {
        Error::OceanAddress(e)
    }
}

impl From<MongoDbError> for Error {
    fn from(e: MongoDbError) -> Error {
        Error::MongoDb(e)
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
            Error::BitcoinHashesHex(ref e) => write!(f, "bitcoin hashes hex error: {}", e),
            Error::Secp256k1(ref e) => write!(f, "secp256k1 error: {}", e),
            Error::OceanAddress(ref e) => write!(f, "ocean address error: {}", e),
            Error::MongoDb(ref e) => write!(f, "mongodb error: {}", e),
            Error::Config(ref e) => write!(f, "config error: {}", e),
            Error::Coordinator(ref e) => write!(f, "coordinator error: {}", e),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::OceanRpc(ref e) => Some(e),
            Error::BitcoinHashes(ref e) => Some(e),
            Error::BitcoinHashesHex(ref e) => Some(e),
            Error::Secp256k1(ref e) => Some(e),
            Error::OceanAddress(ref e) => Some(e),
            Error::MongoDb(ref e) => Some(e),
            Error::Config(ref e) => Some(e),
            Error::Coordinator(_) => None,
        }
    }
}
