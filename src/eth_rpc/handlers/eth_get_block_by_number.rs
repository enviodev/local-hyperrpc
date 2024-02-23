use std::time::Instant;

use crate::types::elapsed;

use super::*;

pub async fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &[RpcRequest],
) -> Vec<RpcResponse> {
    let mut rpc_responses = Vec::new();    

    let start = Instant::now();
    // parse params
    let mut from_blocks_for_txns: Vec<u64> = Vec::new();
    let mut from_blocks_for_headers: Vec<u64> = Vec::new();
    let mut req_ids_with_params: Vec<(i64, u64, bool)> = Vec::new();
    for req in reqs {
        let (block_number, full_txns) =
            match serde_json::from_value::<(RpcBlockNumber, bool)>(req.params.clone()) {
                Ok(res) => res,
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

        if full_txns {
            from_blocks_for_txns.push(from_block);
        } else {
            from_blocks_for_headers.push(from_block);
        }

        req_ids_with_params.push((req.id, from_block, full_txns));
    }

    // optimize query
    let query_ranges_for_txns =
        optimize_query_for_single_block_request(from_blocks_for_txns, rpc_handler.max_block_gap);
    let query_ranges_for_headers =
        optimize_query_for_single_block_request(from_blocks_for_headers, rpc_handler.max_block_gap);


    // execute skar query
    let res_block_txns =
        execute_query_for_block_txns(rpc_handler.skar_client.clone(), query_ranges_for_txns).await;
    let res_block_headers =
        execute_query_for_block_headers(rpc_handler.skar_client.clone(), query_ranges_for_headers).await;

    // if there are any errors, return rpc_responses
    let ((block_txns, metrics0), (block_headers, metrics1)) =
        match (res_block_txns, res_block_headers) {
            (Err(rpc_err), _) | (_, Err(rpc_err)) => {
                for (req_id, _, _) in req_ids_with_params {
                    let rpc_response = rpc_err.to_response(&req_id);
                    rpc_responses.push(rpc_response);
                }
                return (rpc_responses, metrics);
            }
            (Ok(block_txns), Ok(block_headers)) => (block_txns, block_headers),
        };

    metrics += metrics0 + metrics1;

    let start = Instant::now();
    // build responses
    for (req_id, from_block, full_txn) in req_ids_with_params {
        let rpc_result = if full_txn {
            match block_txns.get(&from_block) {
                Some(block) => Ok(RpcResponseData::Block(Some(BlockVariant::Transactions(
                    Box::new(block.clone()),
                )))),
                None => Err(RpcError::InternalError(
                    anyhow!("Block {} not found", from_block).into(),
                )
                .code()),
            }
        } else {
            match block_headers.get(&from_block) {
                Some(block) => Ok(RpcResponseData::Block(Some(BlockVariant::Headers(
                    Box::new(block.clone()),
                )))),
                None => Err(RpcError::InternalError(
                    anyhow!("Block {} not found", from_block).into(),
                )
                .code()),
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
