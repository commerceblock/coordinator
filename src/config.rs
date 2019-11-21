//! # Config
//!
//! Config module handling config options from file/env

use config_rs::{Config as ConfigRs, Environment, File};
use error::InputErrorType::{GenHash, PrivKey};
use serde::{Deserialize, Serialize};
use std::env;
use util::checks::{check_hash_string, check_privkey_string};

use crate::error::{CError, Error, Result};

#[derive(Debug, Serialize, Deserialize)]
/// Api specific config
pub struct ApiConfig {
    /// Client rpc host
    pub host: String,
    /// Client rpc user
    pub user: String,
    /// Client rpc pass
    pub pass: String,
}

impl Default for ApiConfig {
    fn default() -> ApiConfig {
        ApiConfig {
            host: String::new(),
            user: String::new(),
            pass: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Service specific config
pub struct ServiceConfig {
    /// Client rpc host
    pub host: String,
    /// Client rpc user
    pub user: String,
    /// Client rpc pass
    pub pass: String,
}

impl Default for ServiceConfig {
    fn default() -> ServiceConfig {
        ServiceConfig {
            host: String::new(),
            user: String::new(),
            pass: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Clientchain specific config
pub struct ClientChainConfig {
    /// Client rpc host
    pub host: String,
    /// Client rpc user
    pub user: String,
    /// Client rpc pass
    pub pass: String,
    /// Client genesis hash
    pub genesis_hash: String,
    /// Client asset label
    pub asset: String,
    /// Client asset key
    pub asset_key: String,
}

impl Default for ClientChainConfig {
    fn default() -> ClientChainConfig {
        ClientChainConfig {
            host: String::new(),
            user: String::new(),
            pass: String::new(),
            genesis_hash: String::new(),
            asset: String::from("CHALLENGE"),
            asset_key: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Storage specific config
pub struct StorageConfig {
    /// Storage host
    pub host: String,
    /// Storage name
    pub name: String,
    /// Storage user
    pub user: Option<String>,
    /// Storage pass
    pub pass: Option<String>,
}

impl Default for StorageConfig {
    fn default() -> StorageConfig {
        StorageConfig {
            host: String::from("localhost:27017"),
            name: String::from("coordinator"),
            user: None,
            pass: None,
        }
    }
}

/// Config struct storing all config
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Env logger log level
    pub log_level: String,
    /// Challenge duration in seconds
    pub challenge_duration: u64,
    /// Challenge frequency in number of blocks
    pub challenge_frequency: u64,
    /// Verify duration in seconds
    pub verify_duration: u64,
    /// Listener host address
    pub listener_host: String,
    /// Api configuration
    pub api: ApiConfig,
    /// Service configuration
    pub service: ServiceConfig,
    /// Clientchain configuration
    pub clientchain: ClientChainConfig,
    /// Storage configuration
    pub storage: StorageConfig,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            log_level: String::from("coordinator"),
            challenge_duration: 60,
            challenge_frequency: 1,
            verify_duration: 150,
            listener_host: String::from("localhost:80"),
            api: ApiConfig::default(),
            service: ServiceConfig::default(),
            clientchain: ClientChainConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

impl Config {
    /// New Config instance reading default values from value
    /// as well as overriden values by the environment
    pub fn new() -> Result<Self> {
        let mut conf_rs = ConfigRs::new();
        let _ = conf_rs
            // First merge struct default config
            .merge(ConfigRs::try_from(&Config::default())?)?
            // Add in defaults from file config/default.toml if exists
            // This is especially useful for local testing config as
            // the default file is not actually loaded in production
            // This could be done with include_str! if ever required
            .merge(File::with_name("config/default").required(false))?
            // Override any config from env using CO prefix and a
            // "_" separator for the nested config in Config
            .merge(Environment::with_prefix("CO"))?;

        // Override service config from env variables
        // Currently doesn't seem to be supported by config_rs
        // https://github.com/mehcode/config-rs/issues/104
        // A possible alternative would be using a "__" separator
        // e.g. Environment::with_prefix("CO").separator("__")) and
        // setting envs as below but is less readable and confusing
        // CO_CLIENTCHAIN__ASSET_HASH=73be005...
        // CO_CLIENTCHAIN__ASSET=CHALLENGE
        // CO_CLIENTCHAIN__HOST=127.0.0.1:5555
        // CO_CLIENTCHAIN__GENESIS_HASH=706f6...
        if let Ok(v) = env::var("CO_API_HOST") {
            let _ = conf_rs.set("api.host", v)?;
        }
        if let Ok(v) = env::var("CO_API_USER") {
            let _ = conf_rs.set("api.user", v)?;
        }
        if let Ok(v) = env::var("CO_API_PASS") {
            let _ = conf_rs.set("api.pass", v)?;
        }

        if let Ok(v) = env::var("CO_SERVICE_HOST") {
            let _ = conf_rs.set("service.host", v)?;
        }
        if let Ok(v) = env::var("CO_SERVICE_USER") {
            let _ = conf_rs.set("service.user", v)?;
        }
        if let Ok(v) = env::var("CO_SERVICE_PASS") {
            let _ = conf_rs.set("service.pass", v)?;
        }

        if let Ok(v) = env::var("CO_CLIENTCHAIN_HOST") {
            let _ = conf_rs.set("clientchain.host", v)?;
        }
        if let Ok(v) = env::var("CO_CLIENTCHAIN_USER") {
            let _ = conf_rs.set("clientchain.user", v)?;
        }
        if let Ok(v) = env::var("CO_CLIENTCHAIN_PASS") {
            let _ = conf_rs.set("clientchain.pass", v)?;
        }
        if let Ok(v) = env::var("CO_CLIENTCHAIN_ASSET") {
            let _ = conf_rs.set("clientchain.asset", v)?;
        }
        if let Ok(v) = env::var("CO_CLIENTCHAIN_ASSET_KEY") {
            let _ = conf_rs.set("clientchain.asset_key", v)?;
        }
        if let Ok(v) = env::var("CO_CLIENTCHAIN_GENESIS_HASH") {
            let _ = conf_rs.set("clientchain.genesis_hash", v)?;
        }

        if let Ok(v) = env::var("CO_STORAGE_HOST") {
            let _ = conf_rs.set("storage.host", v)?;
        }
        if let Ok(v) = env::var("CO_STORAGE_USER") {
            let _ = conf_rs.set("storage.user", v)?;
        }
        if let Ok(v) = env::var("CO_STORAGE_PASS") {
            let _ = conf_rs.set("storage.pass", v)?;
        }
        if let Ok(v) = env::var("CO_STORAGE_NAME") {
            let _ = conf_rs.set("storage.name", v)?;
        }

        // Perform type checks
        if let Ok(key) = conf_rs.get_str("clientchain.asset_key") {
            if !check_privkey_string(&key) {
                println!("{}", CError::InputError(PrivKey, key.clone()));
                return Err(Error::from(CError::InputError(PrivKey, key)));
            }
        }
        if let Ok(hash) = conf_rs.get_str("clientchain.genesis_hash") {
            if !check_hash_string(&hash) {
                println!("{}", CError::InputError(GenHash, hash.clone()));
                return Err(Error::from(CError::InputError(GenHash, hash)));
            }
        }
        Ok(conf_rs.try_into()?)
    }
}
