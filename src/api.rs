//! Api
//!
//! Api interface for external requests to the coordinator

use std::net::ToSocketAddrs;
use std::str;
use std::sync::Arc;
use std::thread;

use base64::decode;
use bitcoin_hashes::sha256d;
use hyper::{Body, Request, StatusCode};
use jsonrpc_http_server::jsonrpc_core::{Error, ErrorCode, IoHandler, Params, Value};
use jsonrpc_http_server::{hyper::header, AccessControlAllowOrigin, DomainsValidation, Response, ServerBuilder};
use serde::{Deserialize, Serialize};

use crate::challenger::ChallengeResponseIds;
use crate::config::ApiConfig;
use crate::request::{BidSet, Request as ServiceRequest};
use crate::storage::Storage;

#[derive(Deserialize, Debug)]
struct GetRequestParams {
    txid: sha256d::Hash,
}

#[derive(Serialize, Debug)]
struct GetRequestResponse {
    request: ServiceRequest,
    bids: BidSet,
}

/// Get request RPC call returning corresponding request if it exists
fn get_request(params: Params, storage: Arc<dyn Storage>) -> futures::Finished<Value, Error> {
    let try_parse = params.parse::<GetRequestResponsesParams>();
    match try_parse {
        Ok(parse) => {
            let request_get = storage.get_request(parse.txid).unwrap();
            if let Some(request) = request_get {
                let bids = storage.get_bids(request.txid).unwrap();
                let res_serialized = serde_json::to_string(&GetRequestResponse { request, bids }).unwrap();
                return futures::finished(Value::String(res_serialized));
            } else {
                return futures::failed(Error {
                    code: ErrorCode::InvalidParams,
                    message: "Invalid params: `txid` does not exist.".to_string(),
                    data: None,
                });
            }
        }
        Err(e) => return futures::failed(e),
    }
}

#[derive(Serialize, Debug)]
struct GetRequestsResponse {
    requests: Vec<GetRequestResponse>,
}

/// Get requests RPC call returning all stored requests
fn get_requests(storage: Arc<dyn Storage>) -> futures::Finished<Value, Error> {
    let requests = storage.get_requests().unwrap();
    let mut response = GetRequestsResponse { requests: vec![] };
    for request in requests {
        let bids = storage.get_bids(request.txid).unwrap();
        response.requests.push(GetRequestResponse { request, bids })
    }
    return futures::finished(Value::String(serde_json::to_string(&response).unwrap()));
}

#[derive(Deserialize, Debug)]
struct GetRequestResponsesParams {
    txid: sha256d::Hash,
}

#[derive(Serialize, Debug)]
struct GetRequestResponsesResponse {
    responses: Vec<ChallengeResponseIds>,
}

/// Get requests responses RPC call returning all responses for a specific
/// request transaction id hash
fn get_request_responses(params: Params, storage: Arc<dyn Storage>) -> futures::Finished<Value, Error> {
    let try_parse = params.parse::<GetRequestResponsesParams>();
    match try_parse {
        Ok(parse) => {
            let responses = storage.get_responses(parse.txid).unwrap();
            let res_serialized = serde_json::to_string(&GetRequestResponsesResponse { responses }).unwrap();
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
    let storage_ref = storage.clone();
    io.add_method("getrequestresponses", move |params: Params| {
        get_request_responses(params, storage_ref.clone())
    });
    let storage_ref = storage.clone();
    io.add_method("getrequest", move |params: Params| {
        get_request(params, storage_ref.clone())
    });
    io.add_method("getrequests", move |_params| get_requests(storage.clone()));

    let addr: Vec<_> = config
        .host
        .to_socket_addrs()
        .expect("Unable to resolve domain")
        .collect();

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
        .threads(2)
        .start_http(&addr[0])
        .expect("api error");

    thread::spawn(move || server.wait())
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::Future;

    use crate::util::testing::{gen_challenge_state, gen_dummy_hash, MockStorage};

    #[test]
    fn get_request_test() {
        let storage = Arc::new(MockStorage::new());
        let dummy_hash = gen_dummy_hash(1);

        // no such request
        let s = format!(r#"{{"txid": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request(params, storage.clone());
        assert_eq!(
            "Invalid params: `txid` does not exist.",
            resp.wait().unwrap_err().message
        );

        // save actual state
        let state = gen_challenge_state(&dummy_hash);
        storage.save_challenge_state(&state, 0).unwrap();
        let s = format!(r#"{{"txid": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request(params, storage.clone());
        assert_eq!(
            format!(
                r#"{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3"}}]}}"#,
                dummy_hash.to_string()
            ),
            resp.wait().unwrap()
        );
    }

    #[test]
    fn get_requests_test() {
        let storage = Arc::new(MockStorage::new());
        let dummy_hash = gen_dummy_hash(1);

        // no requests
        let resp = get_requests(storage.clone());
        assert_eq!(r#"{"requests":[]}"#, resp.wait().unwrap());

        // save actual state
        let state = gen_challenge_state(&dummy_hash);
        storage.save_challenge_state(&state, 0).unwrap();
        let resp = get_requests(storage.clone());
        assert_eq!(
            format!(
                r#"{{"requests":[{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3"}}]}}]}}"#,
                dummy_hash.to_string()
            ),
            resp.wait().unwrap()
        );

        let dummy_hash2 = gen_dummy_hash(2);
        let state2 = gen_challenge_state(&dummy_hash2);
        storage.save_challenge_state(&state2, 0).unwrap();
        let resp = get_requests(storage.clone());
        assert_eq!(
            format!(
                r#"{{"requests":[{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3"}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3"}}]}}]}}"#,
                dummy_hash.to_string(),
                dummy_hash2.to_string()
            ),
            resp.wait().unwrap()
        );
    }

    #[test]
    fn get_request_responses_test() {
        let storage = Arc::new(MockStorage::new());
        let dummy_hash = gen_dummy_hash(1);
        let dummy_hash_bid = gen_dummy_hash(2);
        let mut dummy_response_set = ChallengeResponseIds::new();
        let _ = dummy_response_set.insert(dummy_hash_bid);
        let _ = storage.save_response(dummy_hash, &dummy_response_set);

        // invalid key
        let s = format!(r#"{{"hash": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request_responses(params, storage.clone());
        assert_eq!(
            "Invalid params: missing field `txid`.",
            resp.wait().unwrap_err().message
        );

        // invalid value
        let s = format!(r#"{{"txid": "{}a"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request_responses(params, storage.clone());
        assert_eq!(
            "Invalid params: bad hex string length 65 (expected 64).",
            resp.wait().unwrap_err().message
        );

        // valid key and value
        let s = format!(r#"{{"txid": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request_responses(params, storage.clone());
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
