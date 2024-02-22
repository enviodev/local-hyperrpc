use super::*;

pub fn handle(
    rpc_handler: Arc<RpcHandler>,
    reqs: &Vec<RpcRequest>,
) -> (Vec<RpcResponse>, QueryMetrics) {
    let mut rpc_responses = Vec::new();

    for req in reqs {
        // not actually a blocknumber, but using BlockNumber to turn the uint chain_id into a hex string
        let rpc_result = Ok(RpcResponseData::BlockNumber(Some(
            rpc_handler.chain_id.into(),
        )));
        rpc_responses.push(RpcResponse::new(
            req.id,
            &rpc_handler.rpc_version,
            rpc_result,
        ));
    }

    (rpc_responses, QueryMetrics::default())
}
