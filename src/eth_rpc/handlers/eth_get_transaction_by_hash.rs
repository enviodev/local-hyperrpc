use std::time::Instant;

use crate::{eth_rpc::types::RpcResult, types::elapsed};

use super::*;

pub async fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let mut rpc_responses: Vec<RpcResponse> = Vec::new();
    let mut metrics = QueryMetrics::default();

    let start = Instant::now();

    let mut tx_hashes: Vec<Hash> = Vec::new();
    let mut req_ids_with_tx_hash: Vec<(i64, Hash)> = Vec::new();
    for req in reqs {
        match serde_json::from_value::<(Hash,)>(req.params.clone()) {
            Ok((tx_hash,)) => {
                tx_hashes.push(tx_hash.clone());
                req_ids_with_tx_hash.push((req.id, tx_hash));
            }
            Err(e) => {
                rpc_responses.push(RpcError::InvalidParams(e.to_string()).to_response(&req.id));
            }
        }
    }

    let (tx_hash_to_block_number, from_blocks): (HashMap<Hash, u64>, Vec<u64>) =
        match rpc_handler.aux_db.map_tx_hashes(&tx_hashes) {
            Ok(block_numbers_and_tx_idxs) => {
                let mut tx_hash_to_block: HashMap<Hash, u64> = HashMap::new();
                let mut from_blocks: Vec<u64> = Vec::new();
                for (maybe_from_block, (_, req_tx_hash)) in block_numbers_and_tx_idxs
                    .iter()
                    .zip(req_ids_with_tx_hash.iter())
                {
                    if let Some((from_block, _)) = maybe_from_block {
                        from_blocks.push(*from_block);
                        tx_hash_to_block.insert(req_tx_hash.clone(), *from_block);
                    }
                }
                (tx_hash_to_block, from_blocks)
            }
            Err(e) => {
                let e = Arc::new(e);
                for (req_id, _) in &req_ids_with_tx_hash {
                    let rpc_response = RpcError::InternalError(e.clone()).to_response(req_id);
                    rpc_responses.push(rpc_response);
                }
                return (rpc_responses, metrics);
            }
        };

    // optimize query
    let query_ranges =
        optimize_query_for_single_block_request(from_blocks, rpc_handler.max_block_gap);

    metrics.query_prepare_time += elapsed(&start);

    // execute query
    let (res_blocks, metrics0) =
        match execute_query_for_block_txns(rpc_handler.state.clone(), query_ranges).await {
            Ok(res) => res,
            Err(rpc_error) => {
                for (req_id, _) in req_ids_with_tx_hash {
                    rpc_responses.push(rpc_error.to_response(&req_id));
                }
                return (rpc_responses, metrics);
            }
        };

    metrics += metrics0;

    let start = Instant::now();

    for (req_id, req_tx_hash) in req_ids_with_tx_hash {
        let rpc_result = extract_rpc_result(&res_blocks, &tx_hash_to_block_number, req_tx_hash);

        rpc_responses.push(RpcResponse::new(
            req_id,
            &rpc_handler.rpc_version,
            rpc_result,
        ));
    }

    metrics.response_encode_time += elapsed(&start);

    (rpc_responses, metrics)
}

fn extract_rpc_result(
    res_blocks: &BTreeMap<u64, Block<Transaction>>,
    tx_hash_to_block_number: &HashMap<Hash, u64>,
    req_tx_hash: Hash,
) -> RpcResult {
    let target_block = if let Some(block_num) = tx_hash_to_block_number.get(&req_tx_hash) {
        block_num
    } else {
        return Ok(RpcResponseData::Transaction(None));
    };

    let block = res_blocks.get(target_block).ok_or_else(|| {
        RpcError::InternalError(anyhow!("Didn't find block that transaction is in").into()).code()
    })?;

    let transaction = block
        .transactions
        .iter()
        .find(|tx| tx.hash == req_tx_hash)
        .cloned();

    Ok(RpcResponseData::Transaction(transaction))
}
