use std::sync::Arc;

use anyhow::{Context, Result};

use crate::config::EthRpcConfig;
use crate::query_handler::QueryHandler;
use crate::rpc_client::RpcClient;

use self::types::{RpcRequest, RpcResponse};

use skar_client::Client as SkarClient;

pub mod types;

pub mod serializer;

pub mod handlers;

pub mod error;

pub struct RpcHandler {
    pub skar_client: SkarClient,
    pub query_handler: QueryHandler,
    pub rpc_client: RpcClient,
    pub hyperrpc_client: RpcClient,
    pub hyperrpc_is_stateful: bool,
    pub rpc_version: String,
    pub chain_id: u64,
    pub max_block_gap: u64,
    pub max_get_logs_block_range: u64,
    pub max_logs_returned_per_request: usize,
    pub max_requests_in_batch: usize,
    pub max_payload_size_in_mb: usize,
}

impl RpcHandler {
    pub fn new(skar_client: SkarClient, rpc_cfg: EthRpcConfig) -> Result<Self> {
        let rpc_client = RpcClient::new("FallbackRPC".to_owned(), rpc_cfg.fallback_url)
            .context("create rpc client")?;

        let hyperrpc_client = RpcClient::new("HyperRPC".to_owned(), rpc_cfg.hyperrpc_url)
            .context("create hyperrpc client")?;

        let query_handler = QueryHandler::new(skar_client.clone());

        Ok(RpcHandler {
            skar_client,
            query_handler,
            rpc_client,
            hyperrpc_client,
            hyperrpc_is_stateful: rpc_cfg.hyperrpc_is_stateful,
            rpc_version: rpc_cfg.json_rpc_version,
            chain_id: rpc_cfg.rpc_chain_id,
            max_block_gap: rpc_cfg.max_block_gap,
            max_get_logs_block_range: rpc_cfg.max_get_logs_block_range,
            max_logs_returned_per_request: rpc_cfg.max_logs_returned_per_request,
            max_payload_size_in_mb: rpc_cfg.max_payload_size_in_mb,
            max_requests_in_batch: rpc_cfg.max_requests_in_batch,
        })
    }

    pub async fn execute_rpc_method(
        self: Arc<Self>,
        method: &str,
        reqs: &Vec<RpcRequest>,
    ) -> Vec<RpcResponse> {
        match method {
            "eth_newFilter"
            | "eth_getFilterLogs"
            | "eth_getFilterChanges"
            | "eth_uninstallFilter"
                if self.hyperrpc_is_stateful =>
            {
                handlers::handle_method_not_found(&self.hyperrpc_client, reqs).await
            }
            "eth_getTransactionByBlockHashAndIndex"
            | "eth_getTransactionByHash"
            | "eth_getBlockByHash"
            | "eth_getTransactionReceipt" => {
                handlers::handle_method_not_found(&self.hyperrpc_client, reqs).await
            }
            "eth_getBlockByNumber" => handlers::eth_get_block_by_number::handle(self, reqs).await,
            "eth_getTransactionByBlockNumberAndIndex" => {
                handlers::eth_get_transaction_by_block_number_and_index::handle(self, reqs).await
            }
            "eth_getBlockReceipts" => handlers::eth_get_block_receipts::handle(self, reqs).await,
            "eth_getLogs" => handlers::eth_get_logs::handle(self, reqs).await,
            "eth_blockNumber" => handlers::eth_block_number::handle(self, reqs).await,
            "eth_chainId" => handlers::eth_chain_id::handle(self, reqs),
            _ => handlers::handle_method_not_found(&self.rpc_client, reqs).await,
        }
    }
}
