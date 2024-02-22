use anyhow::{anyhow, Context, Result};
use skar_format::{Block, BlockNumber, Hash, Trace, Transaction, TransactionReceipt};
use std::result::Result as StdResult;

#[derive(Clone)]
pub enum RpcRequestImpl {
    GetBlockNumber,
    GetBlockByNumber(BlockNumber),
    GetTransactionReceipt(BlockNumber, Hash),
    GetBlockReceipts(BlockNumber),
    TraceBlock(BlockNumber),
}

pub enum RpcResponseImpl {
    GetBlockNumber(BlockNumber),
    GetBlockByNumber(Block<Transaction>),
    GetTransactionReceipt(TransactionReceipt),
    GetBlockReceipts(Vec<TransactionReceipt>),
    TraceBlock(Vec<Trace>),
}

#[derive(Clone)]
pub enum MaybeBatch<T> {
    Single(T),
    Batch(Vec<T>),
}

pub type RpcRequest = MaybeBatch<RpcRequestImpl>;
pub type RpcResponse = MaybeBatch<RpcResponseImpl>;

pub struct GetBlockNumber;

impl From<GetBlockNumber> for RpcRequest {
    fn from(_: GetBlockNumber) -> Self {
        Self::Single(RpcRequestImpl::GetBlockNumber)
    }
}

impl From<GetBlockReceipts> for RpcRequest {
    fn from(req: GetBlockReceipts) -> Self {
        Self::Single(RpcRequestImpl::GetBlockReceipts(req.0))
    }
}

impl TryInto<BlockNumber> for RpcResponseImpl {
    type Error = ();

    fn try_into(self) -> StdResult<BlockNumber, Self::Error> {
        match self {
            RpcResponseImpl::GetBlockNumber(block_num) => Ok(block_num),
            _ => Err(()),
        }
    }
}

pub struct GetBlockByNumber(pub BlockNumber);

impl From<GetBlockByNumber> for RpcRequest {
    fn from(req: GetBlockByNumber) -> Self {
        Self::Single(RpcRequestImpl::GetBlockByNumber(req.0))
    }
}

impl From<Vec<GetBlockByNumber>> for RpcRequest {
    fn from(reqs: Vec<GetBlockByNumber>) -> RpcRequest {
        Self::Batch(
            reqs.into_iter()
                .map(|v| RpcRequestImpl::GetBlockByNumber(v.0))
                .collect(),
        )
    }
}

#[derive(Clone)]
pub struct GetTransactionReceipt(pub BlockNumber, pub Hash);

impl From<Vec<GetTransactionReceipt>> for RpcRequest {
    fn from(reqs: Vec<GetTransactionReceipt>) -> Self {
        Self::Batch(
            reqs.into_iter()
                .map(|v| RpcRequestImpl::GetTransactionReceipt(v.0, v.1))
                .collect(),
        )
    }
}

#[derive(Clone)]
pub struct GetBlockReceipts(pub BlockNumber);

impl From<Vec<GetBlockReceipts>> for RpcRequest {
    fn from(reqs: Vec<GetBlockReceipts>) -> Self {
        Self::Batch(
            reqs.into_iter()
                .map(|v| RpcRequestImpl::GetBlockReceipts(v.0))
                .collect(),
        )
    }
}

impl TryInto<Block<Transaction>> for RpcResponseImpl {
    type Error = ();

    fn try_into(self) -> StdResult<Block<Transaction>, Self::Error> {
        match self {
            RpcResponseImpl::GetBlockByNumber(blocks) => Ok(blocks),
            _ => Err(()),
        }
    }
}

impl<T> TryInto<Vec<T>> for RpcResponse
where
    RpcResponseImpl: TryInto<T, Error = ()>,
{
    type Error = ();

    fn try_into(self) -> StdResult<Vec<T>, Self::Error> {
        match self {
            Self::Batch(resps) => resps.into_iter().map(TryInto::try_into).collect(),
            _ => Err(()),
        }
    }
}

impl TryInto<TransactionReceipt> for RpcResponseImpl {
    type Error = ();

    fn try_into(self) -> StdResult<TransactionReceipt, Self::Error> {
        match self {
            RpcResponseImpl::GetTransactionReceipt(receipt) => Ok(receipt),
            _ => Err(()),
        }
    }
}

impl TryInto<Vec<TransactionReceipt>> for RpcResponseImpl {
    type Error = ();

    fn try_into(self) -> StdResult<Vec<TransactionReceipt>, Self::Error> {
        match self {
            RpcResponseImpl::GetBlockReceipts(receipts) => Ok(receipts),
            _ => Err(()),
        }
    }
}

impl TryInto<Vec<Trace>> for MaybeBatch<RpcResponseImpl> {
    type Error = ();

    fn try_into(self) -> StdResult<Vec<Trace>, Self::Error> {
        match self {
            MaybeBatch::Single(RpcResponseImpl::TraceBlock(traces)) => Ok(traces),
            _ => Err(()),
        }
    }
}

impl RpcResponse {
    pub fn try_into_single<T>(self) -> Option<T>
    where
        RpcResponseImpl: TryInto<T, Error = ()>,
    {
        match self {
            Self::Single(v) => v.try_into().ok(),
            _ => None,
        }
    }
}

impl From<&RpcRequest> for serde_json::Value {
    fn from(req: &RpcRequest) -> serde_json::Value {
        match req {
            RpcRequest::Single(req) => req.to_json(0),
            RpcRequest::Batch(reqs) => {
                let arr = reqs
                    .iter()
                    .enumerate()
                    .map(|(idx, req)| req.to_json(idx))
                    .collect::<Vec<_>>();

                serde_json::Value::Array(arr)
            }
        }
    }
}

impl RpcRequestImpl {
    fn to_json(&self, idx: usize) -> serde_json::Value {
        match self {
            RpcRequestImpl::GetBlockNumber => serde_json::json!({
                "method": "eth_blockNumber",
                "params": [],
                "id": idx,
                "jsonrpc": "2.0",
            }),
            RpcRequestImpl::GetBlockByNumber(block_number) => serde_json::json!({
                "method": "eth_getBlockByNumber",
                "params": [
                    block_number,
                    true,
                ],
                "id": idx,
                "jsonrpc": "2.0",
            }),
            RpcRequestImpl::GetTransactionReceipt(_, hash) => serde_json::json!({
                "method": "eth_getTransactionReceipt",
                "params": [hash],
                "id": idx,
                "jsonrpc": "2.0",
            }),
            RpcRequestImpl::GetBlockReceipts(block_num) => serde_json::json!({
                "method": "eth_getBlockReceipts",
                "params": [block_num],
                "id": idx,
                "jsonrpc": "2.0",
            }),
            RpcRequestImpl::TraceBlock(block_num) => serde_json::json!({
                "method": "trace_block",
                "params": [block_num],
                "id": idx,
                "jsonrpc": "2.0",
            }),
        }
    }
}

impl RpcRequest {
    pub(crate) fn resp_from_json(&self, json: String) -> Result<RpcResponse> {
        let mut json = json.into_bytes();
        let json = simd_json::serde::from_slice(&mut json).context("parse response json")?;

        match (self, json) {
            (Self::Batch(reqs), serde_json::Value::Array(arr)) => {
                let mut vals = Vec::new();

                for (idx, (val, req)) in arr.into_iter().zip(reqs.iter()).enumerate() {
                    match val {
                        serde_json::Value::Object(obj) => {
                            vals.push(req.resp_from_json(idx, obj)?);
                        }
                        _ => return Err(anyhow!("non object item in array response")),
                    }
                }

                Ok(RpcResponse::Batch(vals))
            }
            (Self::Single(req), serde_json::Value::Object(obj)) => {
                Ok(RpcResponse::Single(req.resp_from_json(0, obj)?))
            }
            _ => Err(anyhow!("invalid rpc response")),
        }
    }
}

impl RpcRequestImpl {
    fn resp_from_json(&self, idx: usize, mut json: JsonObject) -> Result<RpcResponseImpl> {
        if json
            .remove("jsonrpc")
            .context("get jsonrpc field")?
            .as_str()
            .context("jsonrpc field is str")?
            != "2.0"
        {
            return Err(anyhow!("invalid jsonrpc field in response"));
        }

        if json
            .remove("id")
            .context("get id field")?
            .as_u64()
            .context("id field is u64")?
            != u64::try_from(idx).unwrap()
        {
            return Err(anyhow!("invalid id field in response"));
        }

        let res = json.remove("result").context("get result field")?;

        match self {
            Self::GetBlockNumber => Ok(RpcResponseImpl::GetBlockNumber(
                serde_json::from_value(res).context("deserialize")?,
            )),
            Self::GetBlockByNumber(_) => serde_json::from_value(res)
                .map(RpcResponseImpl::GetBlockByNumber)
                .context("deserialize"),
            Self::GetTransactionReceipt(_, _) => serde_json::from_value(res)
                .context("deserialize")
                .map(RpcResponseImpl::GetTransactionReceipt),
            Self::GetBlockReceipts(_) => serde_json::from_value(res)
                .context("deserialize")
                .map(RpcResponseImpl::GetBlockReceipts),
            Self::TraceBlock(_) => serde_json::from_value(res)
                .context("deserialize")
                .map(RpcResponseImpl::TraceBlock),
        }
    }
}

type JsonObject = serde_json::Map<String, serde_json::Value>;

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    fn read_json_file(name: &str) -> String {
        std::fs::read_to_string(format!("{}/test-data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn test_get_block_number() {
        let req = RpcRequest::Single(RpcRequestImpl::GetBlockNumber);

        let _: BlockNumber = req
            .resp_from_json(read_json_file("eth_blockNumber.json"))
            .unwrap()
            .try_into_single()
            .unwrap();
    }

    #[test]
    fn test_get_block_by_number() {
        let req = RpcRequest::Batch(vec![
            RpcRequestImpl::GetBlockByNumber(13.into()),
            RpcRequestImpl::GetBlockByNumber(14.into()),
            RpcRequestImpl::GetBlockByNumber(15.into()),
        ]);
        let _: Vec<Block<Transaction>> = req
            .resp_from_json(read_json_file("eth_getBlockByNumber_batch.json"))
            .unwrap()
            .try_into()
            .unwrap();
    }

    #[test]
    fn test_get_transaction_receipt() {
        let req = RpcRequest::Batch(vec![
            RpcRequestImpl::GetTransactionReceipt(
                16929247.into(),
                hex!("017e8ad62f871604544a2ac9ea80ce920a0c79c30f11440a7b481ece7f18b2b0").into(),
            ),
            RpcRequestImpl::GetTransactionReceipt(
                16929247.into(),
                hex!("eab31339e74d34155f8b0a92f384672c7b861c07939f7d58d921d5b50fde640e").into(),
            ),
        ]);
        let _: Vec<TransactionReceipt> = req
            .resp_from_json(read_json_file("eth_getTransactionReceipt_batch.json"))
            .unwrap()
            .try_into()
            .unwrap();
    }
}
