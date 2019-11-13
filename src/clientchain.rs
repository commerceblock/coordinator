//! # ClientChain
//!
//! Client chain interface and implementations

use std::collections::HashMap;

use bitcoin_hashes::{hex::FromHex, sha256d};
use ocean_rpc::{json, RpcApi};

use crate::config::ClientChainConfig;
use crate::error::{CError, Error, Result};
use crate::ocean::OceanClient;

/// Method that returns the first unspent output for given asset
/// or an error if the client wallet does not have any unspent/funds
pub fn get_first_unspent(client: &OceanClient, asset: &str) -> Result<json::ListUnspentResult> {
    // Check asset is held by the wallet and return unspent tx
    let unspent = client.list_unspent(None, None, None, None, Some(asset))?;
    if unspent.is_empty() {
        // TODO: custom error for clientchain
        return Err(Error::from(CError::MissingUnspent(
            String::from(asset),
            String::from("Client"),
        )));
    }
    Ok(unspent[0].clone())
}

/// ClientChain trait defining desired functionality for interfacing
/// with the client chain when coordinating the guardnode service
pub trait ClientChain {
    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<sha256d::Hash>;
    /// Verify challenge transaction has been included in the chain
    fn verify_challenge(&self, txid: &sha256d::Hash) -> Result<bool>;
    /// Get height of client chain
    fn get_blockheight(&self) -> Result<u32>;
}

/// Rpc implementation of Service using an underlying ocean rpc connection
pub struct RpcClientChain<'a> {
    /// Rpc client instance
    client: OceanClient,
    /// Challenge asset id
    asset: &'a str,
}

impl<'a> RpcClientChain<'a> {
    /// Create an RpcClientChain with underlying rpc client connectivity
    pub fn new(clientchain_config: &'a ClientChainConfig) -> Result<Self> {
        let client = OceanClient::new(
            clientchain_config.host.clone(),
            Some(clientchain_config.user.clone()),
            Some(clientchain_config.pass.clone()),
        )?;
        // check we have funds for challenge asset
        match get_first_unspent(&client, &clientchain_config.asset) {
            // If this fails attempt to import the private key and then fetch the unspent again
            Err(_) => {
                client.import_priv_key(&clientchain_config.asset_key, None, None)?;
                if let Err(e) = get_first_unspent(&client, &clientchain_config.asset) {
                    return Err(e);
                }
            }
            _ => (),
        }

        Ok(RpcClientChain {
            client,
            asset: &clientchain_config.asset,
        })
    }
}

impl<'a> ClientChain for RpcClientChain<'a> {
    /// Send challenge transaction to client chain
    fn send_challenge(&self) -> Result<sha256d::Hash> {
        // get any unspent for the challenge asset
        let unspent = get_first_unspent(&self.client, self.asset)?;

        // construct the challenge transaction excluding fees
        // which are not required for policy transactions
        let utxos = vec![json::CreateRawTransactionInput {
            txid: unspent.txid,
            vout: unspent.vout,
            sequence: None,
        }];

        let mut outs = HashMap::new();
        let _ = outs.insert(
            unspent.address.clone(),
            (unspent.amount.into_inner() / 100000000) as f64,
        );

        let mut outs_assets = HashMap::new();
        let _ = outs_assets.insert(unspent.address.clone(), unspent.asset.to_string());

        let tx_hex = self
            .client
            .create_raw_transaction_hex(&utxos, Some(&outs), Some(&outs_assets), None)?;

        // sign the transaction and send via the client rpc
        let tx_signed =
            self.client
                .sign_raw_transaction((&Vec::<u8>::from_hex(&tx_hex)? as &[u8]).into(), None, None, None)?;

        Ok(sha256d::Hash::from_hex(
            &self.client.send_raw_transaction(&tx_signed.hex)?,
        )?)
    }

    /// Verify challenge transaction has been included in the chain
    fn verify_challenge(&self, txid: &sha256d::Hash) -> Result<bool> {
        match self.client.get_raw_transaction_verbose(txid, None) {
            Ok(tx) => {
                // check for blockhash and number of confirmations
                if let (Some(_hash), Some(n_conf)) = (tx.blockhash, tx.confirmations) {
                    if n_conf > 0 {
                        return Ok(true);
                    }
                }
            }
            // no error throwing as issue might have been caused by
            // not successfuly sending the transaction and is not critical
            Err(e) => warn!("verify challenge error{}", e),
        }
        Ok(false)
    }

    /// Return block count of chain
    fn get_blockheight(&self) -> Result<u32> {
        Ok(self.client.get_block_count()? as u32)
    }
}
