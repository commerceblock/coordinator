//! Api
//!
//! Api interface for external requests to the coordinator

use std::net::ToSocketAddrs;
use std::str;
use std::sync::Arc;
use std::thread;

use base64::decode;
use bitcoin::hashes::sha256d;
use hyper::{Body, Request, StatusCode};
use jsonrpc_http_server::jsonrpc_core::{Error, ErrorCode, IoHandler, Params, Value};
use jsonrpc_http_server::{hyper::header, AccessControlAllowOrigin, DomainsValidation, Response, ServerBuilder};
use serde::{Deserialize, Serialize};

use crate::config::ApiConfig;
use crate::interfaces::response::Response as RequestResponse;
use crate::interfaces::storage::Storage;
use crate::interfaces::{bid::BidSet, request::Request as ServiceRequest};

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

#[derive(Deserialize, Debug)]
struct GetRequestsParams {
    page: u64,
}

#[derive(Serialize, Debug)]
struct GetRequestsResponse {
    requests: Vec<GetRequestResponse>,
    pages: u64,
}

/// Default limit on the number of requests returned
static API_REQUESTS_LIMIT: u64 = 10;

/// Get requests RPC call returning all stored requests
fn get_requests(params: Params, storage: Arc<dyn Storage>) -> futures::Finished<Value, Error> {
    let mut page = 1;
    if let Ok(requests_params) = params.parse::<GetRequestsParams>() {
        page = requests_params.page;
    }
    let pages = (storage.get_requests_count().unwrap() as f64 / API_REQUESTS_LIMIT as f64).ceil() as u64;
    let requests = storage
        .get_requests(
            None,
            Some(API_REQUESTS_LIMIT as i64),
            Some(((page - 1) * API_REQUESTS_LIMIT) as i64),
        )
        .unwrap();
    let mut response = GetRequestsResponse {
        requests: vec![],
        pages,
    };
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
struct GetRequestResponseResponse {
    response: RequestResponse,
}

/// Get requests responses RPC call returning all responses for a specific
/// request transaction id hash
fn get_request_response(params: Params, storage: Arc<dyn Storage>) -> futures::Finished<Value, Error> {
    let try_parse = params.parse::<GetRequestResponsesParams>();
    match try_parse {
        Ok(parse) => {
            let response_get = storage.get_response(parse.txid).unwrap();
            if let Some(response) = response_get {
                let res_serialized = serde_json::to_string(&GetRequestResponseResponse { response }).unwrap();
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
    io.add_method("getrequestresponse", move |params: Params| {
        get_request_response(params, storage_ref.clone())
    });
    let storage_ref = storage.clone();
    io.add_method("getrequest", move |params: Params| {
        get_request(params, storage_ref.clone())
    });
    io.add_method("getrequests", move |params: Params| {
        get_requests(params, storage.clone())
    });

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

    use crate::challenger::ChallengeResponseIds;
    use crate::interfaces::mocks::storage::MockStorage;
    use crate::util::testing::{gen_challenge_state, gen_dummy_hash, setup_logger};

    #[test]
    fn get_request_test() {
        setup_logger();
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
        storage.save_challenge_state(&state).unwrap();
        let s = format!(r#"{{"txid": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request(params, storage.clone());
        assert_eq!(
            format!(
                r#"{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}}"#,
                dummy_hash.to_string()
            ),
            resp.wait().unwrap()
        );
    }

    #[test]
    fn get_requests_test() {
        setup_logger();
        let storage = Arc::new(MockStorage::new());
        let dummy_hash = gen_dummy_hash(1);
        let _params = Params::None;

        let s_p1 = format!(r#"{{"page": 1}}"#);
        let params_p1: Params = serde_json::from_str(&s_p1).unwrap();
        let s_m1 = format!(r#"{{"page": -1}}"#);
        let params_m1: Params = serde_json::from_str(&s_m1).unwrap();
        let s_p2 = format!(r#"{{"page": 2}}"#);
        let params_p2: Params = serde_json::from_str(&s_p2).unwrap();
        let s_p5 = format!(r#"{{"page": 5}}"#);
        let params_p5: Params = serde_json::from_str(&s_p5).unwrap();

        // no requests
        let resp = get_requests(Params::None, storage.clone());
        assert_eq!(r#"{"requests":[],"pages":0}"#, resp.wait().unwrap());
        let resp = get_requests(params_p1.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":0}"#, resp.wait().unwrap());
        let resp = get_requests(params_m1.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":0}"#, resp.wait().unwrap());
        let resp = get_requests(params_p2.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":0}"#, resp.wait().unwrap());
        let resp = get_requests(params_p5.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":0}"#, resp.wait().unwrap());

        // save actual state for 1 request
        let state = gen_challenge_state(&dummy_hash);
        storage.save_challenge_state(&state).unwrap();
        let resp_1 = format!(
            r#"{{"requests":[{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}}],"pages":1}}"#,
            dummy_hash.to_string()
        );
        let resp = get_requests(Params::None, storage.clone());
        assert_eq!(resp_1, resp.wait().unwrap());
        let resp = get_requests(params_p1.clone(), storage.clone());
        assert_eq!(resp_1, resp.wait().unwrap());
        let resp = get_requests(params_m1.clone(), storage.clone());
        assert_eq!(resp_1, resp.wait().unwrap());
        let resp = get_requests(params_p2.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":1}"#, resp.wait().unwrap());
        let resp = get_requests(params_p5.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":1}"#, resp.wait().unwrap());

        // save actual state for another request (2 total)
        let dummy_hash2 = gen_dummy_hash(2);
        let state2 = gen_challenge_state(&dummy_hash2);
        storage.save_challenge_state(&state2).unwrap();
        let resp_2 = format!(
            r#"{{"requests":[{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}}],"pages":1}}"#,
            dummy_hash.to_string(),
            dummy_hash2.to_string()
        );
        let resp = get_requests(Params::None, storage.clone());
        assert_eq!(resp_2, resp.wait().unwrap());
        let resp = get_requests(params_p1.clone(), storage.clone());
        assert_eq!(resp_2, resp.wait().unwrap());
        let resp = get_requests(params_m1.clone(), storage.clone());
        assert_eq!(resp_2, resp.wait().unwrap());
        let resp = get_requests(params_p2.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":1}"#, resp.wait().unwrap());
        let resp = get_requests(params_p5.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":1}"#, resp.wait().unwrap());

        // save actual state for 10 more requests (12 total)
        for i in 3..=12 {
            let dummy_hashi = gen_dummy_hash(i);
            let statei = gen_challenge_state(&dummy_hashi);
            storage.save_challenge_state(&statei).unwrap();
        }
        let resp_10 = format!(
            r#"{{"requests":[{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}}],"pages":2}}"#,
            gen_dummy_hash(1).to_string(),
            gen_dummy_hash(2).to_string(),
            gen_dummy_hash(3).to_string(),
            gen_dummy_hash(4).to_string(),
            gen_dummy_hash(5).to_string(),
            gen_dummy_hash(6).to_string(),
            gen_dummy_hash(7).to_string(),
            gen_dummy_hash(8).to_string(),
            gen_dummy_hash(9).to_string(),
            gen_dummy_hash(10).to_string(),
        );
        let resp_12 = format!(
            r#"{{"requests":[{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}},{{"request":{{"txid":"{}","start_blockheight":2,"end_blockheight":5,"genesis_blockhash":"0000000000000000000000000000000000000000000000000000000000000000","fee_percentage":5,"num_tickets":10,"start_blockheight_clientchain":0,"end_blockheight_clientchain":0,"is_payment_complete":false}},"bids":[{{"txid":"1234567890000000000000000000000000000000000000000000000000000000","pubkey":"026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3","payment":null}}]}}],"pages":2}}"#,
            gen_dummy_hash(11).to_string(),
            gen_dummy_hash(12).to_string(),
        );
        let resp = get_requests(Params::None, storage.clone());
        assert_eq!(resp_10, resp.wait().unwrap());
        let resp = get_requests(params_p1.clone(), storage.clone());
        assert_eq!(resp_10, resp.wait().unwrap());
        let resp = get_requests(params_m1.clone(), storage.clone());
        assert_eq!(resp_10, resp.wait().unwrap());
        let resp = get_requests(params_p2.clone(), storage.clone());
        assert_eq!(resp_12, resp.wait().unwrap());
        let resp = get_requests(params_p5.clone(), storage.clone());
        assert_eq!(r#"{"requests":[],"pages":2}"#, resp.wait().unwrap());
    }

    #[test]
    fn get_request_response_test() {
        setup_logger();
        let storage = Arc::new(MockStorage::new());
        let dummy_hash = gen_dummy_hash(1);
        let dummy_hash_bid = gen_dummy_hash(2);

        // no such request
        let s = format!(r#"{{"txid": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request_response(params, storage.clone());
        assert_eq!(
            "Invalid params: `txid` does not exist.",
            resp.wait().unwrap_err().message
        );

        let mut dummy_response_set = ChallengeResponseIds::new();
        let _ = dummy_response_set.insert(dummy_hash_bid);
        let _ = storage.save_response(dummy_hash, &dummy_response_set);

        // invalid key
        let s = format!(r#"{{"hash": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request_response(params, storage.clone());
        assert_eq!(
            "Invalid params: missing field `txid`.",
            resp.wait().unwrap_err().message
        );

        // invalid value
        let s = format!(r#"{{"txid": "{}a"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request_response(params, storage.clone());
        assert_eq!(
            "Invalid params: odd hex string length 65.",
            resp.wait().unwrap_err().message
        );

        // valid key and value
        let s = format!(r#"{{"txid": "{}"}}"#, dummy_hash.to_string());
        let params: Params = serde_json::from_str(&s).unwrap();
        let resp = get_request_response(params, storage.clone());
        assert_eq!(
            format!(
                r#"{{"response":{{"num_challenges":1,"bid_responses":{{"{}":1}}}}}}"#,
                dummy_hash_bid.to_string()
            ),
            resp.wait().unwrap()
        );
    }

    #[test]
    fn authorize_test() {
        setup_logger();
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
