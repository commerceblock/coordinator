//! Api
//!
//! Api interface for external requests to the coordinator

use std::str;
use std::sync::Arc;
use std::thread;

use base64::decode;
use bitcoin_hashes::sha256d;
use hyper::{Body, Request, StatusCode};
use jsonrpc_http_server::jsonrpc_core::{Error, IoHandler, Params, Value};
use jsonrpc_http_server::{hyper::header, AccessControlAllowOrigin, DomainsValidation, Response, ServerBuilder};
use serde::{Deserialize, Serialize};

use crate::challenger::ChallengeResponseIds;
use crate::config::ApiConfig;
use crate::storage::Storage;

#[derive(Deserialize, Debug)]
struct GetChallengeResponsesParams {
    txid: sha256d::Hash,
}

#[derive(Serialize, Debug)]
struct GetChallengeResponsesResponse {
    responses: Vec<ChallengeResponseIds>,
}

/// Get challenge responses RPC call returning all responses for a specific
/// request transaction id hash
fn get_challenge_responses(params: Params, storage: Arc<Storage>) -> futures::Finished<Value, Error> {
    let try_parse = params.parse::<GetChallengeResponsesParams>();
    match try_parse {
        Ok(parse) => {
            let responses = storage.get_all_challenge_responses(parse.txid).unwrap();
            let res_serialized = serde_json::to_string(&GetChallengeResponsesResponse { responses }).unwrap();
            return futures::finished(Value::String(res_serialized));
        }
        Err(e) => return futures::failed(e),
    }
}

/// Do basic authorization on incoming request by parsing the AUTHORIZATION
/// header decoding username/password and comparing with config
fn authorize(our_auth: &str, request: &Request<Body>) -> bool {
    let auth = request
        .headers()
        .get(header::AUTHORIZATION)
        .map(|h| h.to_str().unwrap_or("").to_owned());
    if let Some(auth_basic) = auth {
        let auth_parts: Vec<&str> = auth_basic.split(" ").collect();
        if auth_parts.len() == 2 {
            let auth_basic = &decode(auth_parts[1]).unwrap();
            let auth_basic_str = str::from_utf8(&auth_basic).unwrap();
            return auth_basic_str == our_auth;
        }
    }
    false
}

/// Run Api RPC server for external requests that require information from the
/// coordinator. Data returned to the caller are drawn from the storage
/// interface which is shared with the main coordinator process
pub fn run_api_server<D: Storage + Send + Sync + 'static>(
    config: &ApiConfig,
    storage: Arc<D>,
) -> thread::JoinHandle<()> {
    let mut io = IoHandler::default();
    io.add_method("get_challenge_responses", move |params: Params| {
        get_challenge_responses(params, storage.clone())
    });

    let our_auth = format! {"{}:{}", config.user, config.pass};
    let server = ServerBuilder::new(io)
        .cors(DomainsValidation::AllowOnly(vec![AccessControlAllowOrigin::Null]))
        .request_middleware(move |request: Request<Body>| {
            if our_auth != "" && !authorize(&our_auth, &request) {
                return Response {
                    code: StatusCode::UNAUTHORIZED,
                    content_type: header::HeaderValue::from_str("text/plain").unwrap(),
                    content: "Bad Authorization Attempt".to_string(),
                }
                .into();
            }
            request.into()
        })
        .start_http(&config.host.parse().unwrap())
        .expect("api error");

    thread::spawn(move || server.wait())
}

#[cfg(test)]
mod tests {
    use super::*;

    use bitcoin_hashes::Hash;
    use futures::Future;

    use crate::storage::MockStorage;

    /// Generate dummy hash for tests
    fn gen_dummy_hash(i: u8) -> sha256d::Hash {
        sha256d::Hash::from_slice(&[i as u8; 32]).unwrap()
    }

    #[test]
    fn get_challenge_responses_test() {
        let storage = Arc::new(MockStorage::new());
        let dummy_hash = gen_dummy_hash(1);
        let dummy_hash_bid = gen_dummy_hash(2);
        let mut dummy_response_set = ChallengeResponseIds::new();
        let _ = dummy_response_set.insert(dummy_hash_bid.to_string());
        let _ = storage.save_challenge_responses(dummy_hash, &dummy_response_set);

        // invalid key
        let s = format!(r#"{{"hash": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_challenge_responses(params, storage.clone());
        assert_eq!(
            "Invalid params: missing field `txid`.",
            resp.wait().unwrap_err().message
        );

        // invalid value
        let s = format!(r#"{{"txid": "{}a"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_challenge_responses(params, storage.clone());
        assert_eq!(
            "Invalid params: bad hex string length 65 (expected 64).",
            resp.wait().unwrap_err().message
        );

        // valid key and value
        let s = format!(r#"{{"txid": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_challenge_responses(params, storage.clone());
        assert_eq!(
            format!("{{\"responses\":[[\"{}\"]]}}", dummy_hash_bid.to_string()),
            resp.wait().unwrap()
        );
    }

    #[test]
    fn authorize_test() {
        let our_auth = "user:pass";

        // missing header
        let request: Request<Body> = Request::builder().body(Body::from("")).unwrap();
        assert_eq!(false, authorize(our_auth, &request));

        // incorrect username/password
        let request: Request<Body> = Request::builder()
            .header(
                header::AUTHORIZATION,
                format!("Basic {}", base64::encode("user2:pass1")),
            )
            .body(Body::from(""))
            .unwrap();
        assert_eq!(false, authorize(our_auth, &request));

        // correct username/password
        let request: Request<Body> = Request::builder()
            .header(header::AUTHORIZATION, format!("Basic {}", base64::encode("user:pass")))
            .body(Body::from(""))
            .unwrap();
        assert_eq!(true, authorize(our_auth, &request));
    }
}
