use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::bytes_builder::BytesBuilder;
use crate::eth_rpc::error::RpcError;
use crate::eth_rpc::serializer::parallel_serialize;
use crate::eth_rpc::types::{RpcRequest, RpcRequestErrorCheck, RpcResponse};
use crate::{config::HttpServerConfig, eth_rpc::RpcHandler};

use anyhow::Context;
use axum::body::Body;
use axum::extract::Json as AxumJson;
use axum::extract::State as AxumState;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub struct HttpServer;

pub struct State {
    pub rpc_handler: Arc<RpcHandler>,
    pub cfg: HttpServerConfig,
}

impl HttpServer {
    pub async fn run(rpc_handler: Arc<RpcHandler>, cfg: HttpServerConfig) -> Result<(), anyhow::Error> {

        let addr = cfg.addr;

        let state = Arc::new(State { rpc_handler, cfg });

        let app = axum::Router::new()
    
        
        .route(
            "/",
            axum::routing::post(run_rpc_query).with_state(state.clone()),
        )
        ;

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("bind listener")?;

        axum::serve(listener, app).await.context("run http server")?;

        Ok(())
    }
}


// Make our own error that wraps `anyhow::Error`.
pub struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {:?}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}


pub async fn run_rpc_query(
    AxumState(state): AxumState<Arc<State>>,
    AxumJson(request): AxumJson<serde_json::Value>,
) -> Result<Response, AppError> {
    

    let rpc_handler = state.rpc_handler.clone();

    let mut rpc_responses: Vec<RpcResponse> = Vec::new();

    // deserialize, groups into vec, finds some ParseError
    let (requests_deserialized, batch_flag) = deserialize_req(request);

    let max_number_of_requests = rpc_handler.max_requests_in_batch;
    let requests_param_checked = check_req_fields(
        requests_deserialized,
        max_number_of_requests,
        rpc_handler.rpc_version.clone(),
    );

    let requests_validated = handle_errors(requests_param_checked, &mut rpc_responses);

    // group by method
    let requests_by_method = group_by_method(requests_validated);

    // execute the rpc requests for each method


    for (method, reqs) in &requests_by_method {
        let rpc_handler = rpc_handler.clone();
        let responses = rpc_handler.execute_rpc_method(method, reqs).await;        

        for response in responses {            
            rpc_responses.push(response);
        }
    }
    // sort requests by id
    rpc_responses.sort_by_key(|response| response.id);

    // serialize response
    let serialized_response = serialize_response(
        rpc_responses,
        rpc_handler.max_payload_size_in_mb,
        batch_flag,
    );


    let body = Body::from_stream(futures::stream::iter(
        serialized_response.into_iter().map(Ok::<_, std::io::Error>),
    ));

    let mut response = Response::new(body);

    // insert header
    response.headers_mut().insert(
        "content-type",
        "application/json"
            .try_into()
            .context("Inserting content into response")?,
    );


    Ok(response)
}

fn serialize_response(
    rpc_responses: Vec<RpcResponse>,
    max_response_size: usize,
    batch_flag: bool,
) -> BytesBuilder {
    let serialized_response =
        tokio::task::block_in_place(|| parallel_serialize(rpc_responses, batch_flag));

    let max_response_size_in_bytes: usize = max_response_size * 1_000_000;

    if serialized_response.total_len() > max_response_size_in_bytes {
        let err_response = RpcError::LimitExceeded(format!(
            "Response size larger than {} MB",
            max_response_size
        ))
        .to_response(&0);

        parallel_serialize(vec![err_response], batch_flag)
    } else {
        serialized_response
    }
}


fn deserialize_req(request: serde_json::Value) -> (Vec<RpcRequestErrorCheck>, bool) {
    match request {
        serde_json::Value::Object(request_serialized) => {
            let reqs_deserialized = vec![deserialize_single_req(serde_json::Value::Object(
                request_serialized,
            ))];

            (reqs_deserialized, false)
        }
        serde_json::Value::Array(requests_serialized) => {
            let mut reqs_deserialized: Vec<RpcRequestErrorCheck> = Vec::new();

            for request_serialized in requests_serialized {
                let deserialized = deserialize_single_req(request_serialized);
                reqs_deserialized.push(deserialized);
            }
            (reqs_deserialized, true)
        }
        _ => {
            let err_response = vec![RpcRequestErrorCheck {
                request: RpcRequest::default(),
                error: Some(RpcError::ParseError("Unknown request format".into())),
            }];

            (err_response, false)
        }
    }
}

fn deserialize_single_req(request_serialized: serde_json::Value) -> RpcRequestErrorCheck {
    match serde_json::from_value::<RpcRequest>(request_serialized) {
        Ok(req) => RpcRequestErrorCheck {
            request: req,
            error: None,
        },
        Err(e) => RpcRequestErrorCheck {
            request: RpcRequest::default(),
            error: Some(RpcError::ParseError(e.to_string())),
        },
    }
}

// finds LimitExceeded, JsonRpcVersionNotSupported, DuplicateID errors
fn check_req_fields(
    reqs: Vec<RpcRequestErrorCheck>,
    max_number_of_requests: usize,
    rpc_version: String,
) -> Vec<RpcRequestErrorCheck> {
    if reqs.len() > max_number_of_requests {
        return vec![RpcRequestErrorCheck {
            request: RpcRequest::default(),
            error: Some(RpcError::LimitExceeded(format!(
                "More than {} requests",
                max_number_of_requests
            ))),
        }];
    }

    let mut request_ids: HashSet<i64> = HashSet::new();
    let mut requests_param_checked: Vec<RpcRequestErrorCheck> = Vec::new();
    for req_validated in reqs {
        requests_param_checked.push(match req_validated.error {
            // Some case is for when there is already and error (ParseError from initial deserialization)
            Some(_) => req_validated,
            None => {
                let req = req_validated.request;

                let error_status: Option<RpcError> =
                    // id check
                    if request_ids.contains(&req.id) {
                        Some(RpcError::InvalidParams(format!("duplicate Id: {}", req.id)))
                    // json version check
                    } else if req.jsonrpc != rpc_version {
                        request_ids.insert(req.id);
                        Some(RpcError::JsonRpcVersionNotSupported(req.jsonrpc.clone()))
                    } else {
                        request_ids.insert(req.id);
                        None
                    };

                RpcRequestErrorCheck {
                    request: req,
                    error: error_status,
                }
            }
        });
    }
    requests_param_checked
}

fn handle_errors(
    reqs: Vec<RpcRequestErrorCheck>,
    rpc_responses: &mut Vec<RpcResponse>,
) -> Vec<RpcRequest> {
    let mut valid_requests: Vec<RpcRequest> = Vec::new();

    for req_validated in reqs {
        if let Some(rpc_error) = req_validated.error {
            rpc_responses.push(rpc_error.to_response(&req_validated.request.id));
        } else {
            valid_requests.push(req_validated.request);
        }
    }

    valid_requests
}

// groups all requests by handler (either an error or method)
fn group_by_method(reqs: Vec<RpcRequest>) -> HashMap<String, Vec<RpcRequest>> {
    let mut reqs_by_method: HashMap<String, Vec<RpcRequest>> = HashMap::new();
    for req in reqs {
        reqs_by_method
            .entry(req.method.clone())
            .or_default()
            .push(req);
    }

    reqs_by_method
}
