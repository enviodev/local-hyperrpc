use serde::{Deserialize, Serialize};
use std::num::{NonZeroU64, NonZeroUsize};
use url::Url;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcClientConfig {
    #[serde(default = "default_req_timeout")]
    pub http_req_timeout_millis: NonZeroU64,
    pub endpoints: Vec<EndpointConfig>,
}

pub fn default_req_timeout() -> NonZeroU64 {
    NonZeroU64::new(30000).unwrap()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EndpointConfig {
    pub url: Url,
    pub bearer_token: Option<String>,
    #[serde(default = "default_refresh_interval")]
    pub status_refresh_interval_secs: NonZeroU64,
    #[serde(flatten)]
    pub limit: LimitConfig,
    pub label: Option<String>,
}

pub fn default_refresh_interval() -> NonZeroU64 {
    NonZeroU64::new(1).unwrap()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LimitConfig {
    #[serde(default = "default_req_limit")]
    pub req_limit: NonZeroUsize,
    #[serde(default = "default_limit_window")]
    pub req_limit_window_ms: NonZeroU64,
    #[serde(default = "default_batch_size_limit")]
    pub batch_size_limit: NonZeroUsize,
}

pub fn default_req_limit() -> NonZeroUsize {
    NonZeroUsize::new(10).unwrap()
}

pub fn default_limit_window() -> NonZeroU64 {
    NonZeroU64::new(1000).unwrap()
}

pub fn default_batch_size_limit() -> NonZeroUsize {
    NonZeroUsize::new(50).unwrap()
}
