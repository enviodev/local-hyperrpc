use super::*;

pub async fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let mut rpc_responses = Vec::new();

    let latest_block = match resolve_latest_block(&rpc_handler.state.height()) {
        Ok(block) => block,
        Err(rpc_error) => {
            for i in reqs {
                rpc_responses.push(rpc_error.to_response(&i.id));
            }
            return (rpc_responses, QueryMetrics::default());
        }
    };

    for req in reqs {
        let rpc_result = Ok(RpcResponseData::BlockNumber(Some((latest_block).into())));
        rpc_responses.push(RpcResponse::new(
            req.id,
            &rpc_handler.rpc_version,
            rpc_result,
        ))
    }

    (rpc_responses, QueryMetrics::default())
}
