//! # Config
//!
//! Config module handling config options from file/env

use config_rs::{Config as ConfigRs, Environment, File};
use serde::Deserialize;
use std::env;

use crate::error::Result;

#[derive(Debug, Deserialize)]
/// Clientchain specific config
pub struct ClientChainConfig {
    /// Client rpc host
    pub host: String,
    /// Client rpc user
    pub user: String,
    /// Client rpc pass
    pub pass: String,
    /// Client asset
    pub asset: String,
    /// Client asset hash
    pub asset_hash: String,
    /// Client genesis hash
    pub genesis_hash: String,
}

#[derive(Debug, Deserialize)]
/// Storage specific config
pub struct StorageConfig {
    /// Storage host
    pub host: String,
    /// Storage user
    pub user: String,
    /// Storage pass
    pub pass: String,
    /// Storage name
    pub name: String,
}

/// Config struct storing all config
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Env logger log level
    pub log_level: String,
    /// Challenge duration
    pub challenge_duration: u64,
    /// Verify duration
    pub verify_duration: u64,
    /// Listener host address
    pub listener_host: String,
    /// Clientchain configuration
    pub clientchain: ClientChainConfig,
    /// Storage configuration
    pub storage: StorageConfig,
}

impl Config {
    /// New Config instance reading default values from value
    /// as well as overriden values by the environment
    pub fn new() -> Result<Self> {
        let mut conf_rs = ConfigRs::new();
        let _ = conf_rs
            // Add in defaults from file config/default.toml
            .merge(File::with_name("config/default"))?
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
