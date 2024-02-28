use super::*;

pub async fn handle(rpc_handler: Arc<RpcHandler>, reqs: &Vec<RpcRequest>) -> Vec<RpcResponse> {
    let mut rpc_responses = Vec::new();

    // parse params
    let mut block_ranges: Vec<BlockRange> = Vec::new();
    let mut log_filter_data_with_req_ids_validated: Vec<LogFilterDataWithReqId> = Vec::new();
    for req in reqs {
        let params = match serde_json::from_value::<(FilterParams,)>(req.params.clone()) {
            Ok((params,)) => params,
            Err(e) => {
                rpc_responses.push(RpcError::InvalidParams(e.to_string()).to_response(&req.id));
                continue;
            }
        };

        let log_filter = match params.parse_into_log_filter(&rpc_handler).await {
            Ok(log_filter) => log_filter,
            Err(rpc_error) => {
                rpc_responses.push(rpc_error.to_response(&req.id));
                continue;
            }
        };

        // we don't care about filter_id or log_filter.next_poll_block_number
        // we just want this struct for composability
        let mimic = LogFilterDataWithReqId {
            log_filter: log_filter.clone(),
            req_id: req.id,
        };
        log_filter_data_with_req_ids_validated.push(mimic);
        block_ranges.push(BlockRange(log_filter.from_block, log_filter.to_block));
    }

    let (successful_request_info, logs_tree) = concurrent_batch_skar_log_query(
        rpc_handler.skar_client.clone(),
        rpc_handler.max_logs_returned_per_request,
        rpc_handler.max_get_logs_block_range,
        log_filter_data_with_req_ids_validated,
        block_ranges,
        &mut rpc_responses,
    )
    .await;

    for log_filter_data_with_req_id in successful_request_info {
        // let log_filter = log_filter_with_req_id.log_filter;
        let log_selection = log_filter_data_with_req_id.log_filter.selection;
        let from_block = log_filter_data_with_req_id.log_filter.from_block;
        // +1 because range is exclusive but the request is inclusive
        let to_block = log_filter_data_with_req_id.log_filter.to_block + 1;
        let req_id = log_filter_data_with_req_id.req_id;

        trim_log_tree_into_response(
            &logs_tree,
            from_block,
            to_block,
            log_selection,
            req_id,
            &mut rpc_responses,
            &rpc_handler.rpc_version,
        );
    }

    rpc_responses
}
