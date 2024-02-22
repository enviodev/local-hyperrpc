use anyhow::Context;
use arrayvec::ArrayVec;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use skar_format::{
    Address, Block, BlockNumber, Hash, Log, LogArgument, Transaction, TransactionReceipt,
};
use skar_net_types::LogSelection;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use super::error::{RpcError, RpcErrorCode};
use super::handlers::resolve_block_number;
use super::RpcHandler;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RpcBlockNumber {
    BlockNumber(BlockNumber),
    Latest,
    Earliest,
}

impl<'de> Deserialize<'de> for RpcBlockNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(RpcBlockNumberVisitor)
    }
}

struct RpcBlockNumberVisitor;

impl<'de> Visitor<'de> for RpcBlockNumberVisitor {
    type Value = RpcBlockNumber;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("BlockNumber, 'latest', or 'earliest'")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match value {
            "latest" => Ok(RpcBlockNumber::Latest),
            "earliest" => Ok(RpcBlockNumber::Earliest),
            _ => BlockNumber::from_str(value)
                .map_err(|e| E::custom(e.to_string()))
                .map(RpcBlockNumber::BlockNumber),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FilterParams {
    pub from_block: Option<RpcBlockNumber>,
    pub to_block: Option<RpcBlockNumber>,
    pub address: Option<SingleOrMultiple<Address>>,
    pub topics: Option<ArrayVec<SingleOrMultiple<LogArgument>, 4>>,
    pub block_hash: Option<Hash>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum SingleOrMultiple<T> {
    Single(T),
    Multiple(Vec<T>),
}

impl<T> SingleOrMultiple<T> {
    pub fn turn_into_vec(self) -> Vec<T> {
        match self {
            SingleOrMultiple::Single(x) => vec![x],
            SingleOrMultiple::Multiple(x) => x,
        }
    }
}

impl FilterParams {
    pub fn parse_into_log_filter(self, rpc_handler: &RpcHandler) -> Result<LogFilter, RpcError> {
        let archive_height = rpc_handler.state.height();

        let (from_block, to_block) = if let Some(block_hash) = self.block_hash {
            return Err(RpcError::InvalidParams(
                "Blockhash not implemented yet"
                    .into(),
            ));
        } else {
            (
                resolve_block_number(self.from_block, &archive_height)?,
                // the filter's to_block is inclusive but the skar query is exclusive
                resolve_block_number(self.to_block, &archive_height)? + 1,
            )
        };

        let address = match self.address {
            Some(x) => x.turn_into_vec(),
            None => Vec::new(),
        };
        let topics = match self.topics {
            Some(y) => {
                let mut out: ArrayVec<Vec<LogArgument>, 4> = ArrayVec::new();
                for x in y {
                    out.push(x.turn_into_vec());
                }
                out
            }
            None => ArrayVec::new(),
        };
        // seems like infura uses to_block for this.
        // +1 because eth_getFilterLogs gets up to including to_block,
        // so eth_getFilterChanges should get the rest
        let next_poll_block_number = to_block + 1;
        Ok(LogFilter {
            selection: LogSelection { address, topics },
            from_block,
            to_block,
            next_poll_block_number,
        })
    }
}

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RpcRequest {
    pub id: i64,
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RpcResponse {
    pub id: i64,
    pub jsonrpc: String,
    pub result: RpcResult,
}

pub type RpcResult = Result<RpcResponseData, RpcErrorCode>;

impl RpcResponse {
    pub fn new(id: i64, jsonrpc: &str, result: RpcResult) -> Self {
        RpcResponse {
            id,
            jsonrpc: jsonrpc.into(),
            result,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum RpcResponseData {
    Block(Option<BlockVariant>),
    Logs(Option<Vec<Log>>),
    Receipts(Option<Vec<TransactionReceipt>>),
    SingleReceipt(Option<TransactionReceipt>),
    BlockNumber(Option<BlockNumber>),
    Transaction(Option<Transaction>),
    FilterId(FilterId),
    UninstallFilter(bool),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum BlockVariant {
    Transactions(Box<Block<Transaction>>),
    Headers(Box<Block<Hash>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcRequestErrorCheck {
    pub request: RpcRequest,
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone)]
pub struct FilterIdWithReqId {
    pub filter_id: FilterId,
    pub req_id: i64,
}

#[derive(Debug, Clone)]
pub struct LogFilterDataWithReqId {
    pub log_filter: LogFilter,
    pub filter_id: FilterId,
    pub req_id: i64,
}
