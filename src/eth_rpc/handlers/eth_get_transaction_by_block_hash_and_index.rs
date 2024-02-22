use crate::{eth_rpc::types::RpcResult, types::elapsed};

use std::time::Instant;

use super::*;

pub async fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let rpc_version = &rpc_handler.rpc_version;
    let mut rpc_responses: Vec<RpcResponse> = Vec::new();
    let mut metrics = QueryMetrics::default();

    let start = Instant::now();
    // parse params
    let mut block_hashes: Vec<Hash> = Vec::new();
    let mut req_ids_with_hash_and_txn_idx: Vec<(i64, Hash, u64)> = Vec::new();
    for req in reqs {
        match serde_json::from_value::<(Hash, BlockNumber)>(req.params.clone()) {
            Ok((block_hash, txn_idx)) => {
                block_hashes.push(block_hash.clone());
                req_ids_with_hash_and_txn_idx.push((req.id, block_hash, txn_idx.into()));
            }
            Err(e) => {
                rpc_responses.push(RpcError::InvalidParams(e.to_string()).to_response(&req.id));
            }
        }
    }

    let block_numbers = match rpc_handler.aux_db.map_block_hashes(&block_hashes) {
        Ok(block_numbers) => block_numbers,
        Err(e) => {
            let e = Arc::new(e);
            for (req_id, _, _) in req_ids_with_hash_and_txn_idx {
                let rpc_response = RpcError::InternalError(e.clone()).to_response(&req_id);
                rpc_responses.push(rpc_response);
            }
            return (rpc_responses, metrics);
        }
    };

    let mut block_hash_to_block_number: HashMap<Hash, u64> = HashMap::new();
    let mut from_blocks: Vec<u64> = Vec::new();
    for (maybe_from_block, (_, block_hash, _)) in block_numbers
        .iter()
        .zip(req_ids_with_hash_and_txn_idx.iter())
    {
        if let Some(from_block) = maybe_from_block {
            block_hash_to_block_number.insert(block_hash.clone(), *from_block);
            from_blocks.push(*from_block);
        }
    }

    // optimize query
    let query_ranges =
        optimize_query_for_single_block_request(from_blocks, rpc_handler.max_block_gap);

    metrics.query_prepare_time += elapsed(&start);

    // execute query
    let (res_blocks, metrics0) =
        match execute_query_for_block_txns(rpc_handler.state.clone(), query_ranges).await {
            Ok(res) => res,
            Err(rpc_err) => {
                for (req_id, _, _) in req_ids_with_hash_and_txn_idx {
                    let response = rpc_err.to_response(&req_id);
                    rpc_responses.push(response);
                }
                return (rpc_responses, metrics);
            }
        };

    metrics += metrics0;

    let start = Instant::now();
    for (req_id, block_hash, txn_idx) in req_ids_with_hash_and_txn_idx {
        let rpc_result = extract_rpc_result(
            &block_hash_to_block_number,
            &res_blocks,
            txn_idx,
            &block_hash,
        );
        rpc_responses.push(RpcResponse::new(req_id, rpc_version, rpc_result));
    }
    metrics.response_encode_time += elapsed(&start);

    (rpc_responses, metrics)
}

fn extract_rpc_result(
    block_hash_to_block_number: &HashMap<Hash, u64>,
    res_blocks: &BTreeMap<u64, Block<Transaction>>,
    req_txn_idx: u64,
    req_block_hash: &Hash,
) -> RpcResult {
    let block_number = match block_hash_to_block_number.get(req_block_hash) {
        Some(block_number) => block_number,
        None => {
            return Ok(RpcResponseData::Transaction(None));
        }
    };

    let block = match res_blocks.get(block_number) {
        Some(block) => {
            // make sure the loaded block is the block requested
            if block.header.hash != *req_block_hash {
                return Ok(RpcResponseData::Transaction(None));
            } else {
                block
            }
        }
        None => {
            return Err(RpcError::InternalError(
                anyhow!("Didn't find block that transaction is in").into(),
            )
            .code());
        }
    };

    let response_data = match block
        .transactions
        .iter()
        .find(|txn| txn.transaction_index == req_txn_idx.into())
    {
        Some(txn) => RpcResponseData::Transaction(Some(txn.clone())),
        None => RpcResponseData::Transaction(None),
    };

    Ok(response_data)
}
