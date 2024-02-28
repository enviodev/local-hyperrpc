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
    // parse params
    let filter_id_with_req_ids = parse_param_filter_id(reqs, &mut rpc_responses);

    let log_filter_with_req_ids =
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

    let mut block_ranges: Vec<BlockRange> = Vec::new();
    let mut log_filter_data_with_req_ids_validated: Vec<LogFilterDataWithReqId> = Vec::new();
    for maybe_log_filter_and_req_id in &log_filter_with_req_ids {
        match &maybe_log_filter_and_req_id.0 {
            Ok(log_filter_with_req_id) => {
                log_filter_data_with_req_ids_validated.push(log_filter_with_req_id.clone());
                let log_filter = &log_filter_with_req_id.log_filter;
                block_ranges.push(BlockRange(log_filter.from_block, log_filter.to_block));
            }
            Err(rpc_error) => {
                let err_response = rpc_error.to_response(&maybe_log_filter_and_req_id.1);
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

    metrics.response_encode_time += elapsed(&start);

    (rpc_responses, metrics)
}
