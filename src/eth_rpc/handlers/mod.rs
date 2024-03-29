use crate::query_handler::from_arrow::batch_to_logs;
use crate::query_handler::QueryHandler;
use crate::rpc_client::{self, RpcClient, RpcRequestImpl};
use crate::BlockRange;

use super::error::RpcError;
use super::types::{
    BlockVariant, FilterParams, LogFilterDataWithReqId, RpcBlockNumber, RpcRequest, RpcResponse,
    RpcResponseData,
};
use super::RpcHandler;
use anyhow::Result;

use arrayvec::ArrayVec;
use futures::{Future, StreamExt};
use skar_net_types::FieldSelection;
use skar_net_types::LogSelection;
use skar_net_types::Query;
use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{anyhow, Context, Error};

use skar_format::{Block, BlockNumber, Hash, Log, LogArgument, Transaction, TransactionReceipt};

pub mod eth_block_number;
pub mod eth_chain_id;
pub mod eth_get_block_by_number;
pub mod eth_get_block_receipts;
pub mod eth_get_logs;
pub mod eth_get_transaction_by_block_number_and_index;

// various helper and shared methods

pub async fn handle_method_not_found(
    rpc_client: &RpcClient,
    reqs_validated: &[RpcRequest],
) -> Vec<RpcResponse> {
    let mut resps = Vec::new();

    for chunk in reqs_validated.chunks(50) {
        let chunk = chunk
            .iter()
            .map(|req| RpcRequestImpl::Proxy {
                params: req.params.clone(),
                method: req.method.clone(),
            })
            .collect::<Vec<_>>();

        let req = rpc_client::RpcRequest::Batch(chunk);

        // TODO: actually handle error
        let r = rpc_client.send(req).await.unwrap();

        let r: Vec<serde_json::Value> = r.try_into().unwrap();

        resps.extend_from_slice(&r);
    }

    resps
        .into_iter()
        .zip(reqs_validated.iter())
        .map(|(res, req)| RpcResponse {
            id: req.id,
            jsonrpc: req.jsonrpc.clone(),
            result: Ok(RpcResponseData::Proxy(res)),
        })
        .collect()
}

fn select_logs(logs: &[Log], selection: LogSelection) -> Vec<Log> {
    // returns a cloned subset of the vec of logs that match the LogSelection
    let mut logs_res: Vec<Log> = Vec::new();
    for log in logs {
        let address_match =
            selection.address.is_empty() || selection.address.contains(&log.address);
        let topics_match = match_topics(&selection.topics, &log.topics);
        if address_match && topics_match {
            logs_res.push(log.clone());
        }
    }
    logs_res
}

fn match_topics(
    selection_topics: &ArrayVec<Vec<LogArgument>, 4>,
    log_topics: &ArrayVec<LogArgument, 4>,
) -> bool {
    for i in 0..4 {
        let log_argument = &log_topics.get(i);
        let selection_arguments = &selection_topics.get(i);

        if let (Some(log_argument), Some(selection_arguments)) = (log_argument, selection_arguments)
        {
            if !selection_arguments.is_empty() && !selection_arguments.contains(log_argument) {
                return false;
            }
        }
    }

    true
}

fn trim_log_tree_into_response(
    logs_tree: &BTreeMap<u64, Vec<Log>>,
    from_block: u64,
    to_block: u64,
    log_selection: LogSelection,
    req_id: i64,
    rpc_responses: &mut Vec<RpcResponse>,
    json_rpc_version: &str,
) {
    // get the vec of logs from tree then apply the filter
    let logs: Vec<Log> = logs_tree
        .range(from_block..to_block)
        .flat_map(|(_, logs)| logs.iter().cloned())
        .collect();

    let selected_logs = select_logs(&logs, log_selection);

    let rpc_result = if selected_logs.is_empty() {
        Ok(RpcResponseData::Logs(None))
    } else {
        Ok(RpcResponseData::Logs(Some(selected_logs)))
    };

    let rpc_response = RpcResponse::new(req_id, json_rpc_version, rpc_result);

    rpc_responses.push(rpc_response);
}

async fn single_skar_log_query(
    client: skar_client::Client,
    max_logs_per_request: usize,
    log_selection: LogSelection,
    block_range: BlockRange,
) -> anyhow::Result<Vec<Log>> {
    let log_field_selection = FieldSelection {
        log: skar_schema::log()
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect(),
        ..Default::default()
    };

    let query = Query {
        from_block: block_range.0,
        // +1 since skar query is exclusive
        to_block: Some(block_range.1 + 1),
        logs: vec![log_selection],
        field_selection: log_field_selection,
        max_num_logs: Some(max_logs_per_request),
        ..Default::default()
    };

    let query_res = client
        .send::<skar_client::ArrowIpc>(&query)
        .await
        .context("send skar query")?;

    let mut num_logs_returned = 0;
    let mut logs_res: Vec<Log> = Vec::new();
    for arrow_batch in query_res.data.logs {
        for log in batch_to_logs(&arrow_batch).context("arrow data to logs")? {
            if num_logs_returned >= max_logs_per_request {
                return Err(anyhow!(format!(
                    "More than {} logs returned",
                    max_logs_per_request
                )));
            }
            logs_res.push(log);
            num_logs_returned += 1;
        }
    }

    // make sure on the final query result that the next_block is the `to_block` that I initially passed in
    // otherwise the skar query timed out
    if query_res.next_block < block_range.1 && query_res.next_block != 0 {
        return Err(anyhow!("Query timed out"));
    }

    Ok(logs_res)
}

async fn concurrent_batch_skar_log_query(
    client: skar_client::Client,
    max_logs_per_request: usize,
    max_get_logs_block_range: u64,
    requested_log_data: Vec<LogFilterDataWithReqId>,
    requested_block_ranges: Vec<BlockRange>,
    rpc_responses: &mut Vec<RpcResponse>,
) -> (Vec<LogFilterDataWithReqId>, BTreeMap<u64, Vec<Log>>) {
    let mut futures = Vec::new();

    let mut valid_requested_log_data: Vec<LogFilterDataWithReqId> = Vec::new();
    for (request_data, block_range) in requested_log_data.iter().zip(requested_block_ranges) {
        if block_range.1 - block_range.0 > max_get_logs_block_range {
            let rpc_response = RpcError::LimitExceeded(format!(
                "Requested block range is greater than {}",
                max_get_logs_block_range
            ))
            .to_response(&request_data.req_id);
            rpc_responses.push(rpc_response);
        } else {
            let log_selection = request_data.log_filter.selection.clone();
            let future = single_skar_log_query(
                client.clone(),
                max_logs_per_request,
                log_selection,
                block_range,
            );
            valid_requested_log_data.push(request_data.clone());
            futures.push(future);
        }
    }

    let mut successful_requested_log_data = Vec::new();
    let mut logs_tree: BTreeMap<u64, Vec<Log>> = BTreeMap::new();
    let logs_res = join_buffered(futures.into_iter(), CONCURRENCY).await;
    for (query_res, requested_data) in logs_res.into_iter().zip(valid_requested_log_data.iter()) {
        match query_res {
            Ok(logs) => {
                // add logs to tree
                for log in logs {
                    logs_tree
                        .entry(*log.block_number)
                        .or_default()
                        .push(log.clone());
                }
                successful_requested_log_data.push(requested_data.clone());
            }
            Err(e) => {
                if e.to_string().contains("More than") {
                    rpc_responses.push(
                        RpcError::LimitExceeded(e.to_string()).to_response(&requested_data.req_id),
                    )
                } else {
                    rpc_responses
                        .push(RpcError::InternalError(e.into()).to_response(&requested_data.req_id))
                }
            }
        }
    }

    (successful_requested_log_data, logs_tree)
}

pub fn resolve_block_number(
    block_number_param: Option<RpcBlockNumber>,
    archive_height: &anyhow::Result<Option<u64>>,
) -> Result<u64, RpcError> {
    match block_number_param {
        Some(block_number) => {
            let latest_block = resolve_latest_block(archive_height)?;

            match block_number {
                RpcBlockNumber::BlockNumber(block_number) => {
                    if *block_number > latest_block {
                        Err(RpcError::InvalidParams(format!(
                            "requested block {} is greater than latest block {}",
                            *block_number, latest_block
                        )))
                    } else {
                        Ok(*block_number)
                    }
                }
                RpcBlockNumber::Earliest => Ok(0),
                RpcBlockNumber::Latest => Ok(latest_block),
            }
        }
        None => Ok(resolve_latest_block(archive_height)?),
    }
}

fn resolve_latest_block(archive_height: &anyhow::Result<Option<u64>>) -> Result<u64, RpcError> {
    match archive_height {
        Ok(Some(block_number)) => Ok(*block_number),
        Ok(None) => Err(RpcError::InternalError(
            anyhow!("Latest block not found").into(),
        )),
        Err(e) => Err(RpcError::InternalError(anyhow!("{:?}", e).into())),
    }
}

fn optimize_query_for_single_block_request(
    mut from_blocks: Vec<u64>,
    max_block_gap: u64,
) -> Vec<BlockRange> {
    from_blocks.sort();

    let mut query_ranges: Vec<BlockRange> = Vec::new();
    let mut iter = from_blocks.iter();
    let mut curr_from_block = match iter.next() {
        Some(block) => block,
        None => return Vec::new(),
    };
    let mut current_query_range_index = 0;
    query_ranges.push(BlockRange(*curr_from_block, *curr_from_block + 1)); // initial query batch
    for next_from_block in iter {
        // since it's sorted next should always be >= curr so it won't be negative.
        // if next_from_block is less than or equal to MAX_BLOCK_GAP blocks away from current, add it to the batch
        if next_from_block - curr_from_block <= max_block_gap {
            // add to current query range
            let old_query_range = query_ranges[current_query_range_index];
            let new_query_range = BlockRange(old_query_range.0, *next_from_block + 1);
            query_ranges[current_query_range_index] = new_query_range;
        } else {
            // start a new query range
            let new_query_batch = BlockRange(*next_from_block, *next_from_block + 1);
            query_ranges.push(new_query_batch);
            current_query_range_index += 1;
        }
        curr_from_block = next_from_block;
    }

    query_ranges
}

async fn execute_query_for_block_receipts(
    handler: QueryHandler,
    query_ranges: Vec<BlockRange>,
) -> Result<BTreeMap<(u64, u64), TransactionReceipt>, RpcError> {
    let mut futures = Vec::new();

    for block_range in query_ranges {
        let handler = handler.clone();
        let single_res_receipts = async move { handler.get_block_receipts(block_range).await };
        futures.push(single_res_receipts);
    }

    let resp = try_join_buffered(futures.into_iter(), CONCURRENCY)
        .await
        .map_err(|e: Error| RpcError::InternalError(e.into()))?;

    let mut resps = BTreeMap::new();

    for res in resp {
        resps.extend(res);
    }

    Ok(resps)
}

async fn execute_query_for_block_txns(
    handler: QueryHandler,
    query_ranges: Vec<BlockRange>,
) -> Result<BTreeMap<u64, Block<Transaction>>, RpcError> {
    let mut futures = Vec::new();
    for block_range in query_ranges {
        let handler = handler.clone();
        let single_res_block_with_transaction =
            async move { handler.get_blocks_with_transactions(block_range).await };

        futures.push(single_res_block_with_transaction);
    }

    let resp = try_join_buffered(futures.into_iter(), CONCURRENCY)
        .await
        .map_err(|e| RpcError::InternalError(e.into()))?;

    let mut resps = BTreeMap::new();

    for res in resp {
        resps.extend(res);
    }

    Ok(resps)
}

async fn execute_query_for_block_headers(
    handler: QueryHandler,
    query_ranges: Vec<BlockRange>,
) -> Result<BTreeMap<u64, Block<Hash>>, RpcError> {
    let mut futures = Vec::new();
    for block_range in query_ranges {
        let handler = handler.clone();
        let single_res_block_with_transaction =
            async move { handler.get_blocks(block_range).await };

        futures.push(single_res_block_with_transaction);
    }

    let resp = try_join_buffered(futures.into_iter(), CONCURRENCY)
        .await
        .map_err(|e| RpcError::InternalError(e.into()))?;

    let mut resps = BTreeMap::new();

    for res in resp {
        resps.extend(res);
    }

    Ok(resps)
}

const CONCURRENCY: usize = 4;

async fn join_buffered<I, F, T>(futs: I, buffer_size: usize) -> Vec<T>
where
    F: Future<Output = T>,
    I: Iterator<Item = F>,
{
    let mut stream = futures::stream::iter(futs).buffered(buffer_size);

    let mut res = Vec::new();
    while let Some(v) = stream.next().await {
        res.push(v);
    }

    res
}

async fn try_join_buffered<I, F, T>(futs: I, buffer_size: usize) -> Result<Vec<T>>
where
    F: Future<Output = Result<T>>,
    I: Iterator<Item = F>,
{
    let mut stream = futures::stream::iter(futs).buffered(buffer_size);

    let mut res = Vec::new();
    while let Some(v) = stream.next().await {
        match v {
            Ok(v) => res.push(v),
            Err(e) => return Err(e),
        }
    }

    Ok(res)
}
