use crate::types::elapsed;

use std::time::Instant;

use super::*;

pub async fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let rpc_version = &rpc_handler.rpc_version;
    let mut rpc_responses = Vec::new();

    let mut metrics = QueryMetrics::default();

    let start = Instant::now();
    let mut from_blocks: Vec<u64> = Vec::new();
    let mut req_ids_with_blocks: Vec<(i64, u64)> = Vec::new();
    for req in reqs {
        let block_number = match serde_json::from_value::<(RpcBlockNumber,)>(req.params.clone()) {
            Ok((block_number,)) => block_number,
            Err(e) => {
                rpc_responses.push(RpcError::InvalidParams(e.to_string()).to_response(&req.id));
                continue;
            }
        };

        let from_block = match resolve_block_number(Some(block_number), &rpc_handler.state.height())
        {
            Ok(from_block) => from_block,
            Err(rpc_error) => {
                rpc_responses.push(rpc_error.to_response(&req.id));
                continue;
            }
        };

        from_blocks.push(from_block);
        req_ids_with_blocks.push((req.id, from_block));
    }

    // optimize query
    let query_ranges =
        optimize_query_for_single_block_request(from_blocks, rpc_handler.max_block_gap);

    metrics.query_prepare_time += elapsed(&start);

    // execute queries
    let (receipts, metrics0) =
        match execute_query_for_block_receipts(rpc_handler.state.clone(), query_ranges).await {
            Ok(receipts) => receipts,
            Err(rpc_error) => {
                for (req_id, _) in req_ids_with_blocks {
                    let response = rpc_error.to_response(&req_id);
                    rpc_responses.push(response);
                }
                return (rpc_responses, metrics);
            }
        };
    metrics += metrics0;

    let start = Instant::now();
    // combine inner BTreeMap on blockNumber
    let mut res_receipts_by_block: BTreeMap<u64, Vec<TransactionReceipt>> = BTreeMap::new();
    for ((block_number, _), receipt) in receipts {
        res_receipts_by_block
            .entry(block_number)
            .or_default()
            .push(receipt);
    }

    for (req_id, from_block) in req_ids_with_blocks {
        let rpc_result = match res_receipts_by_block.get(&from_block) {
            Some(receipts) => Ok(RpcResponseData::Receipts(Some(receipts.clone()))),
            None => Ok(RpcResponseData::Receipts(None)),
        };

        rpc_responses.push(RpcResponse::new(req_id, rpc_version, rpc_result));
    }
    metrics.response_encode_time += elapsed(&start);

    (rpc_responses, metrics)
}
