use std::time::Instant;

use crate::types::elapsed;

use super::*;

pub async fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let mut rpc_responses = Vec::new();
    let mut metrics = QueryMetrics::default();

    let start = Instant::now();

    let mut block_hash_for_txns: Vec<Hash> = Vec::new();
    let mut block_hash_for_headers: Vec<Hash> = Vec::new();
    let mut hash_to_req_id: HashMap<Hash, i64> = HashMap::new();
    let mut req_ids_with_params: Vec<(i64, Hash, bool)> = Vec::new();
    for req in reqs {
        let (block_hash, full_txns) =
            match serde_json::from_value::<(Hash, bool)>(req.params.clone()) {
                Ok(res) => res,
                Err(e) => {
                    rpc_responses.push(RpcError::InvalidParams(e.to_string()).to_response(&req.id));
                    continue;
                }
            };

        if full_txns {
            block_hash_for_txns.push(block_hash.clone());
        } else {
            block_hash_for_headers.push(block_hash.clone());
        }

        req_ids_with_params.push((req.id, block_hash.clone(), full_txns));
        hash_to_req_id.insert(block_hash.clone(), req.id);
    }

    let from_blocks_for_txns = match rpc_handler.aux_db.map_block_hashes(&block_hash_for_txns) {
        Ok(block_numbers) => block_numbers.into_iter().flatten().collect(),
        Err(e) => {
            let e = Arc::new(e);
            for (req_id, _, _) in req_ids_with_params {
                let result = Err(RpcError::InternalError(e.clone()).code());
                rpc_responses.push(RpcResponse::new(req_id, &rpc_handler.rpc_version, result));
            }
            return (rpc_responses, metrics);
        }
    };

    let from_blocks_for_headers: Vec<u64> =
        match rpc_handler.aux_db.map_block_hashes(&block_hash_for_headers) {
            Ok(block_hashes) => block_hashes.into_iter().flatten().collect(),
            Err(e) => {
                let e = Arc::new(e);
                for block_hash in block_hash_for_headers {
                    let result = Err(RpcError::InternalError(e.clone()).code());
                    let req_id = *hash_to_req_id.get(&block_hash).unwrap_or(&0);
                    rpc_responses.push(RpcResponse::new(req_id, &rpc_handler.rpc_version, result));
                }
                return (rpc_responses, metrics);
            }
        };

    // optimize query
    let query_ranges_for_txns =
        optimize_query_for_single_block_request(from_blocks_for_txns, rpc_handler.max_block_gap);
    let query_ranges_for_headers =
        optimize_query_for_single_block_request(from_blocks_for_headers, rpc_handler.max_block_gap);

    metrics.query_prepare_time += elapsed(&start);

    // execute skar query
    let res_block_txns =
        execute_query_for_block_txns(rpc_handler.state.clone(), query_ranges_for_txns).await;
    let res_block_headers =
        execute_query_for_block_headers(rpc_handler.state.clone(), query_ranges_for_headers).await;

    // if there are any errors, return rpc_responses
    let ((block_txns, metrics0), (block_headers, metrics1)) =
        match (res_block_txns, res_block_headers) {
            (Err(rpc_err), Err(_)) | (Err(rpc_err), Ok(_)) | (Ok(_), Err(rpc_err)) => {
                for i in req_ids_with_params {
                    let rpc_response = rpc_err.to_response(&i.0);
                    rpc_responses.push(rpc_response);
                }
                return (rpc_responses, metrics);
            }
            (Ok(block_txns), Ok(block_headers)) => (block_txns, block_headers),
        };

    metrics += metrics0 + metrics1;

    let start = Instant::now();
    // turn from btree block_num->block into map block_hash->block
    let mut hash_to_block_txn = HashMap::new();
    for block in block_txns.values() {
        let hash = &block.header.hash;
        hash_to_block_txn.insert(hash, block.clone());
    }
    let mut hash_to_block_header = HashMap::new();
    for block in block_headers.values() {
        let hash = &block.header.hash;
        hash_to_block_header.insert(hash, block.clone());
    }

    // build responses
    for (req_id, req_block_hash, full_txn) in req_ids_with_params {
        let rpc_result = if full_txn {
            match hash_to_block_txn.get(&req_block_hash) {
                Some(block) => Ok(RpcResponseData::Block(Some(BlockVariant::Transactions(
                    Box::new(block.clone()),
                )))),
                None => Err(RpcError::InvalidParams("Block not found".to_string()).code()),
            }
        } else {
            match hash_to_block_header.get(&req_block_hash) {
                Some(block) => Ok(RpcResponseData::Block(Some(BlockVariant::Headers(
                    Box::new(block.clone()),
                )))),
                None => Err(RpcError::InvalidParams("Block not found".to_string()).code()),
            }
        };

        rpc_responses.push(RpcResponse::new(
            req_id,
            &rpc_handler.rpc_version,
            rpc_result,
        ));
    }

    metrics.response_encode_time += elapsed(&start);

    (rpc_responses, metrics)
}
