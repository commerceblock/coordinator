//! # Ocean
//!
//! Ocean node communication implementations

use ocean_rpc::{Client, RpcApi};

use crate::error::Result;

/// Extension of ocean_rpc::Client that retries rpc calls
pub struct RpcClient {
    /// Ocean rpc client instance
    pub client: Client,
}

impl RpcClient {
    /// Create an RpcClient with underlying rpc client connectivity
    pub fn new(url: String, user: Option<String>, pass: Option<String>) -> Result<Self> {
        Ok(RpcClient {
            client: Client::new(url, user, pass),
        })
    }
}

/// Interval between retry attempts of rpc client
pub const CLIENT_INTERVAL: u64 = 10;

/// Number of retry attemps for rpc client calls
pub const CLIENT_RETRY_ATTEMPTS: u8 = 5;

impl RpcApi for RpcClient {
    fn call<T: for<'b> serde::de::Deserialize<'b>>(
        &self,
        cmd: &str,
        args: &[serde_json::Value],
    ) -> ocean_rpc::Result<T> {
        for _ in 0..CLIENT_RETRY_ATTEMPTS {
            match self.client.call(cmd, args) {
                Ok(ret) => return Ok(ret),
                Err(ocean_rpc::Error::JsonRpc(e)) => {
                    warn!("rpc error: {}, retrying...", e);
                    ::std::thread::sleep(::std::time::Duration::from_millis(CLIENT_INTERVAL));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        self.client.call(cmd, args)
    }
}
