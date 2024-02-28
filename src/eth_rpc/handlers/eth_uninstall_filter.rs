use super::*;

pub fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let mut rpc_responses: Vec<RpcResponse> = Vec::new();
    let metrics = QueryMetrics::default();

    let filter_and_req_ids = parse_param_filter_id(reqs, &mut rpc_responses);

    let filter_ids: Vec<FilterId> = filter_and_req_ids
        .iter()
        .map(|i| i.filter_id.clone())
        .collect();

    match rpc_handler.aux_db.delete_filters(&filter_ids) {
        Ok(_) => (),
        Err(e) => {
            let e = Arc::new(e);
            for i in filter_and_req_ids {
                let rpc_result = RpcError::InternalError(e.clone()).to_response(&i.req_id);
                rpc_responses.push(rpc_result);
            }
            return (rpc_responses, metrics);
        }
    };

    for i in filter_and_req_ids {
        let rpc_result = Ok(RpcResponseData::UninstallFilter(true));

        rpc_responses.push(RpcResponse::new(
            i.req_id,
            &rpc_handler.rpc_version,
            rpc_result,
        ));
    }

    (rpc_responses, metrics)
}
