use super::*;

pub async fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let mut rpc_responses: Vec<RpcResponse> = Vec::new();
    let metrics = QueryMetrics::default();

    let mut req_ids: Vec<i64> = Vec::new();
    let mut filters: Vec<LogFilter> = Vec::new();
    for req in reqs {
        let params = match serde_json::from_value::<(FilterParams,)>(req.params.clone()) {
            Ok((params,)) => params,
            Err(e) => {
                rpc_responses.push(RpcError::InvalidParams(e.to_string()).to_response(&req.id));
                continue;
            }
        };

        let log_filter = match params.parse_into_log_filter(&rpc_handler) {
            Ok(log_filter) => log_filter,
            Err(rpc_error) => {
                rpc_responses.push(rpc_error.to_response(&req.id));
                continue;
            }
        };

        req_ids.push(req.id);
        filters.push(log_filter);
    }

    let filter_ids: Vec<FilterId> = match rpc_handler.aux_db.save_filters(&filters) {
        Ok(filter_ids) => filter_ids,
        Err(e) => {
            let e = Arc::new(e);
            for req_id in req_ids {
                let rpc_response = RpcError::InternalError(e.clone()).to_response(&req_id);
                rpc_responses.push(rpc_response);
            }
            return (rpc_responses, metrics);
        }
    };

    for (req_id, filter_id) in req_ids.into_iter().zip(filter_ids.into_iter()) {
        let rpc_result = Ok(RpcResponseData::FilterId(filter_id));
        rpc_responses.push(RpcResponse::new(
            req_id,
            &rpc_handler.rpc_version,
            rpc_result,
        ));
    }

    (rpc_responses, metrics)
}
