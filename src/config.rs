//! # Config
//!
//! Config module handling config options from file/env

use config_rs::{Config as ConfigRs, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;

use crate::error::Result;

#[derive(Debug, Serialize, Deserialize)]
/// Clientchain specific config
pub struct ClientChainConfig {
    /// Client rpc host
    pub host: String,
    /// Client rpc user
    pub user: String,
    /// Client rpc pass
    pub pass: String,
    /// Client asset hash
    pub asset_hash: String,
    /// Client genesis hash
    pub genesis_hash: String,
    /// Client asset label
    pub asset: String,
}

impl Default for ClientChainConfig {
    fn default() -> ClientChainConfig {
        ClientChainConfig {
            host: String::from(""),
            user: String::from(""),
            pass: String::from(""),
            asset_hash: String::from(""),
            genesis_hash: String::from(""),
            asset: String::from("CHALLENGE"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
    /// Verify duration in seconds
    pub verify_duration: u64,
    /// Listener host address
    pub listener_host: String,
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
            verify_duration: 150,
            listener_host: String::from("localhost:80"),
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
            // This is especially useful for local testing config
            .merge(File::with_name("config/default").required(false))?
            // Override any config from env
            .merge(Environment::with_prefix("CO"))?;

        // Override clientchain config from env variables
        // Currently doesn't seem to be supported by config_rs
        // https://github.com/mehcode/config-rs/issues/104
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
        if let Ok(v) = env::var("CO_CLIENTCHAIN_ASSET_HASH") {
            let _ = conf_rs.set("clientchain.asset_hash", v)?;
        }
        if let Ok(v) = env::var("CO_CLIENTCHAIN_GENESIS_HASH") {
            let _ = conf_rs.set("clientchain.genesis_hash", v)?;
        }

        // Override storage config from env variables
        // Currently doesn't seem to be supported by config_rs
        // https://github.com/mehcode/config-rs/issues/104
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

        Ok(conf_rs.try_into()?)
    }
}
