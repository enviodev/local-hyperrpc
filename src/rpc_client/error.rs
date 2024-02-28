use std::result::Result as StdResult;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("Failed to execute http request:\n{0}")]
    HttpRequest(reqwest::Error),
    #[error("None of the endpoints can handle this rpc request. {0:#?}")]
    NoHealthyEndpoints(Vec<Self>),
    #[error("Endpoint limit is too low to handle this request.")]
    EndpointLimitTooLow,
    #[error("Endpoint is too behind to handle the request.")]
    EndpointTooBehind,
    #[error("Endpoint is unavailable. Client failed to get height of it.")]
    EndpointUnavailable,
    #[error("Invalid RPC response.\n{0:?}")]
    InvalidRPCResponse(anyhow::Error),
    #[error("All retries failed when trying to execute RPC request.")]
    RetriesFailed,
}

pub type Result<T> = StdResult<T, Error>;
