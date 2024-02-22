use std::time::Instant;

use crate::types::elapsed;

use super::*;

pub async fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let mut rpc_responses: Vec<RpcResponse> = Vec::new();
    let mut metrics = QueryMetrics::default();

    let start = Instant::now();
    let mut txn_hashes: Vec<Hash> = Vec::new();
    let mut txn_hash_and_req_id: Vec<(Hash, i64)> = Vec::new();
    for req in reqs {
        let txn_hash = match serde_json::from_value::<(Hash,)>(req.params.clone()) {
            Ok((txn_hash,)) => txn_hash,
            Err(e) => {
                rpc_responses.push(RpcError::InvalidParams(e.to_string()).to_response(&req.id));
                continue;
            }
        };

        txn_hashes.push(txn_hash.clone());
        txn_hash_and_req_id.push((txn_hash.clone(), req.id));
    }

    let block_numbers_and_tx_idxs = match rpc_handler.aux_db.map_tx_hashes(&txn_hashes) {
        Ok(block_numbers_and_tx_idxs) => block_numbers_and_tx_idxs,
        Err(e) => {
            let e = Arc::new(e);
            for (_, req_id) in txn_hash_and_req_id {
                let rpc_response = RpcError::InternalError(e.clone()).to_response(&req_id);
                rpc_responses.push(rpc_response);
            }
            return (rpc_responses, metrics);
        }
    };

    let mut tx_hash_to_block_number_and_txn_idx: HashMap<Hash, (u64, u64)> = HashMap::new();
    let mut res_block_numbers_and_tx_idxs: Vec<(u64, u64)> = Vec::new();
    for i in 0..block_numbers_and_tx_idxs.len() {
        let maybe_txn_hash = txn_hashes.get(i);
        let maybe_block_number_and_tx_idx = block_numbers_and_tx_idxs.get(i);

        if let (Some(txn_hash), Some(maybe_block_number_and_tx_idx)) =
            (maybe_txn_hash, maybe_block_number_and_tx_idx)
        {
            let res_txn_hash: Hash = txn_hash.clone();
            if let Some(block_number_and_tx_idx) = maybe_block_number_and_tx_idx {
                res_block_numbers_and_tx_idxs.push(*block_number_and_tx_idx);
                tx_hash_to_block_number_and_txn_idx.insert(res_txn_hash, *block_number_and_tx_idx);
            }
        }
    }

    let from_blocks: Vec<u64> = res_block_numbers_and_tx_idxs
        .iter()
        .map(|(block_num, _)| *block_num)
        .collect();

    // optimize query
    let query_ranges =
        optimize_query_for_single_block_request(from_blocks, rpc_handler.max_block_gap);

    metrics.query_prepare_time += elapsed(&start);

    // execute queries
    let (res_receipts, metrics0) =
        match execute_query_for_block_receipts(rpc_handler.state.clone(), query_ranges).await {
            Ok(receipts) => receipts,
            Err(rpc_error) => {
                for (_, req_id) in txn_hash_and_req_id {
                    let response = rpc_error.to_response(&req_id);
                    rpc_responses.push(response);
                }
                return (rpc_responses, metrics);
            }
        };

    metrics += metrics0;

    let start = Instant::now();

    for (txn_hash, req_id) in txn_hash_and_req_id {
        // get block number and index for the hash
        let rpc_result = match tx_hash_to_block_number_and_txn_idx.get(&txn_hash) {
            Some(block_num_and_txn_idx) => match res_receipts.get(block_num_and_txn_idx) {
                Some(txn_receipt) => Ok(RpcResponseData::SingleReceipt(Some(txn_receipt.clone()))),
                None => Ok(RpcResponseData::SingleReceipt(None)),
            },
            None => Ok(RpcResponseData::SingleReceipt(None)),
        };

        let rpc_response = RpcResponse::new(req_id, &rpc_handler.rpc_version, rpc_result);
        rpc_responses.push(rpc_response);
    }

    metrics.response_encode_time += elapsed(&start);

    (rpc_responses, metrics)
}
