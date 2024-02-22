use std::time::Instant;

use crate::types::elapsed;

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
    let filter_id_with_req_ids = parse_param_filter_id(reqs, &mut rpc_responses);

    let log_filter_data_with_req_ids =
        match get_filters(rpc_handler.aux_db.clone(), &filter_id_with_req_ids) {
            Ok(res) => res,
            Err(e) => {
                let e = Arc::new(e);
                for i in filter_id_with_req_ids {
                    let err_response = RpcError::InternalError(e.clone()).to_response(&i.req_id);
                    rpc_responses.push(err_response);
                }
                return (rpc_responses, metrics);
            }
        };

    let latest_block = match resolve_latest_block(&rpc_handler.state.height()) {
        Ok(latest_block) => latest_block,
        Err(rpc_error) => {
            for i in filter_id_with_req_ids {
                let rpc_result = rpc_error.to_response(&i.req_id);
                rpc_responses.push(rpc_result);
            }
            return (rpc_responses, metrics);
        }
    };

    let mut block_ranges: Vec<BlockRange> = Vec::new();
    let mut log_filter_data_with_req_ids_validated: Vec<LogFilterDataWithReqId> = Vec::new();
    for maybe_log_filter_data_with_req_id in &log_filter_data_with_req_ids {
        match &maybe_log_filter_data_with_req_id.0 {
            Ok(log_filter_data_with_req_id) => {
                log_filter_data_with_req_ids_validated.push(log_filter_data_with_req_id.clone());
                let log_filter = &log_filter_data_with_req_id.log_filter;
                block_ranges.push(BlockRange(log_filter.next_poll_block_number, latest_block));
            }
            Err(rpc_error) => {
                let err_response = rpc_error.to_response(&maybe_log_filter_data_with_req_id.1);
                rpc_responses.push(err_response);
            }
        }
    }

    metrics.query_prepare_time += elapsed(&start);

    // execute skar queries
    let (successful_request_info, logs_tree, metrics0) = concurrent_batch_skar_log_query(
        rpc_handler.state.clone(),
        rpc_handler.max_logs_returned_per_request,
        rpc_handler.max_get_logs_block_range,
        log_filter_data_with_req_ids_validated,
        block_ranges,
        &mut rpc_responses,
    )
    .await;

    metrics += metrics0;

    let start = Instant::now();

    // get results for each filter
    for log_filter_with_req_id in &successful_request_info {
        let log_selection = log_filter_with_req_id.log_filter.selection.clone();
        let from_block = log_filter_with_req_id.log_filter.next_poll_block_number;
        // +1 because range is exclusive but the request is inclusive
        let to_block = latest_block + 1;
        let req_id = log_filter_with_req_id.req_id;

        trim_log_tree_into_response(
            &logs_tree,
            from_block,
            to_block,
            log_selection,
            req_id,
            &mut rpc_responses,
            rpc_version,
        );
    }

    // collect filters to update
    let mut new_poll_block_number: Vec<u64> = Vec::new();
    let mut filter_ids_to_update: Vec<FilterId> = Vec::new();
    for i in &successful_request_info {
        filter_ids_to_update.push(i.filter_id.clone());
        // + 1 because we just got latest block in this query
        // on next poll we want to start after latest block
        new_poll_block_number.push(latest_block + 1);
    }

    // update filter
    if let Err(e) = rpc_handler
        .aux_db
        .update_filters_poll_block_number(&filter_ids_to_update, &new_poll_block_number)
    {
        let e = Arc::new(e);
        let rpc_result = Err(RpcError::InternalError(e.clone()).code());
        for i in &successful_request_info {
            rpc_responses.push(RpcResponse::new(i.req_id, rpc_version, rpc_result.clone()));
        }
        return (rpc_responses, metrics);
    }
    metrics.response_encode_time += elapsed(&start);

    (rpc_responses, metrics)
}
