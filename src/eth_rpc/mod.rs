use std::sync::Arc;

use crate::{config::EthRpcConfig};

use self::handlers::eth_block_number;
use self::types::{RpcRequest, RpcResponse};

use skar_client::Client as SkarClient;

pub mod types;

pub mod serializer;

pub mod handlers;

pub mod error;

pub struct RpcHandler {
    pub client: SkarClient,
    pub rpc_version: String,
    pub chain_id: u64,
    pub max_block_gap: u64,
    pub max_get_logs_block_range: u64,
    pub max_logs_returned_per_request: usize,
    pub max_requests_in_batch: usize,
    pub max_payload_size_in_mb: usize,
}

impl RpcHandler {
    pub fn new(client: SkarClient, rpc_cfg: EthRpcConfig) -> Self {
        RpcHandler {
            client,
            rpc_version: rpc_cfg.json_rpc_version,
            chain_id: rpc_cfg.rpc_chain_id,
            max_block_gap: rpc_cfg.max_block_gap,
            max_get_logs_block_range: rpc_cfg.max_get_logs_block_range,
            max_logs_returned_per_request: rpc_cfg.max_logs_returned_per_request,
            max_payload_size_in_mb: rpc_cfg.max_payload_size_in_mb,
            max_requests_in_batch: rpc_cfg.max_requests_in_batch,
        }
    }

    pub async fn execute_rpc_method(
        self: Arc<Self>,
        method: &str,
        reqs: &Vec<RpcRequest>,
    ) -> Vec<RpcResponse> {
        match method {
            // "eth_getBlockByNumber" => eth_get_block_by_number::handle(self, reqs).await,
            // "eth_getTransactionByBlockNumberAndIndex" => {
            //     eth_get_transaction_by_block_number_and_index::handle(self, reqs).await
            // }
            // "eth_getTransactionByBlockHashAndIndex" => {
            //     eth_get_transaction_by_block_hash_and_index::handle(self, reqs).await
            // }
            // "eth_getTransactionByHash" => eth_get_transaction_by_hash::handle(self, reqs).await,
            // "eth_getBlockByHash" => eth_get_block_by_hash::handle(self, reqs).await,
            // "eth_getTransactionReceipt" => eth_get_transaction_receipt::handle(self, reqs).await,
            // "eth_getBlockReceipts" => eth_get_block_receipts::handle(self, reqs).await,
            // "eth_getLogs" => eth_get_logs::handle(self, reqs).await,
            // "eth_newFilter" => eth_new_filter::handle(self, reqs).await,
            // "eth_getFilterLogs" => eth_get_filter_logs::handle(self, reqs).await,
            // "eth_getFilterChanges" => eth_get_filter_changes::handle(self, reqs).await,
            // "eth_uninstallFilter" => eth_uninstall_filter::handle(self, reqs),
            "eth_blockNumber" => eth_block_number::handle(self, reqs).await,
            // "eth_chainId" => eth_chain_id::handle(self, reqs),
            _ => handle_method_not_found(reqs),
        }
    }
}
