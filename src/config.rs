use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub eth_rpc: EthRpcConfig,
    pub http_server: HttpServerConfig,
    pub hypersync: skar_client::Config,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpServerConfig {
    pub addr: SocketAddr,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EthRpcConfig {
    /// Url to hyperrpc
    pub hyperrpc_url: String,
    ///  Maximum number of requests in a batch request
    #[serde(default = "default_max_requests_in_batch")]
    pub max_requests_in_batch: usize,
    ///  Maximum range between to_block and from_block fields for an eth_getLogs request
    #[serde(default = "default_max_get_logs_block_range")]
    pub max_get_logs_block_range: u64,
    ///  for optimizing skar queries.  max gap between blocks to be split into separate skar queries during optimization
    #[serde(default = "default_max_block_gap")]
    pub max_block_gap: u64,
    /// max number of logs that can be returned in a single eth_getLogs, eth_getFilterLogs, or eth_getFilterChanges
    #[serde(default = "default_max_logs_returned_per_request")]
    pub max_logs_returned_per_request: usize,
    ///  Maximum payload size to return to client in MB
    #[serde(default = "default_max_payload_size_in_mb")]
    pub max_payload_size_in_mb: usize,
    /// supported json version
    #[serde(default = "default_json_rpc_version")]
    pub json_rpc_version: String,
    //  temp for chain_id
    pub rpc_chain_id: u64,
}

fn default_max_requests_in_batch() -> usize {
    500
}

fn default_max_get_logs_block_range() -> u64 {
    69_000_000_000
}

fn default_max_block_gap() -> u64 {
    100
}

fn default_max_logs_returned_per_request() -> usize {
    50_000
}

fn default_max_payload_size_in_mb() -> usize {
    150
}

fn default_json_rpc_version() -> String {
    "2.0".into()
}
