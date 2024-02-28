use crate::eth_rpc::types::RpcResult;

use super::*;

pub async fn handle(rpc_handler: Arc<RpcHandler>, reqs: &Vec<RpcRequest>) -> Vec<RpcResponse> {
    let mut rpc_responses: Vec<RpcResponse> = Vec::new();

    // parse params
    let mut from_blocks: Vec<u64> = Vec::new();
    let mut req_ids_with_block_num_and_tx_idx: Vec<(i64, u64, u64)> = Vec::new();

    for req in reqs {
        let (block_number, tx_index) =
            match serde_json::from_value::<(RpcBlockNumber, BlockNumber)>(req.params.clone()) {
                Ok(res) => res,
                Err(e) => {
                    rpc_responses.push(RpcError::InvalidParams(e.to_string()).to_response(&req.id));
                    continue;
                }
            };

        let from_block = match resolve_block_number(
            Some(block_number),
            &rpc_handler.skar_client.get_height().await.map(Some),
        ) {
            Ok(from_block) => from_block,
            Err(rpc_error) => {
                rpc_responses.push(rpc_error.to_response(&req.id));
                continue;
            }
        };

        from_blocks.push(from_block);
        req_ids_with_block_num_and_tx_idx.push((req.id, from_block, tx_index.into()));
    }

    // optimize query
    let query_ranges =
        optimize_query_for_single_block_request(from_blocks, rpc_handler.max_block_gap);

    // execute query
    let res_blocks =
        match execute_query_for_block_txns(rpc_handler.query_handler.clone(), query_ranges).await {
            Ok(res) => res,
            Err(rpc_err) => {
                for (req_id, _, _) in req_ids_with_block_num_and_tx_idx {
                    let response = rpc_err.to_response(&req_id);
                    rpc_responses.push(response);
                }
                return rpc_responses;
            }
        };

    for (req_id, from_block, tx_index) in req_ids_with_block_num_and_tx_idx {
        let rpc_result = extract_rpc_result(&res_blocks, from_block, tx_index);

        rpc_responses.push(RpcResponse::new(
            req_id,
            &rpc_handler.rpc_version,
            rpc_result,
        ));
    }

    rpc_responses
}

fn extract_rpc_result(
    res_blocks: &BTreeMap<u64, Block<Transaction>>,
    from_block: u64,
    tx_index: u64,
) -> RpcResult {
    let block = match res_blocks.get(&from_block) {
        Some(block) => block,
        None => {
            return Err(
                RpcError::InternalError(anyhow!("Block {} not found", from_block).into()).code(),
            );
        }
    };

    // find transaction in block
    let response_data = match block
        .transactions
        .iter()
        .find(|tx| tx.transaction_index == tx_index.into())
    {
        Some(tx) => RpcResponseData::Transaction(Some(tx.clone())),
        None => RpcResponseData::Transaction(None),
    };

    Ok(response_data)
}
