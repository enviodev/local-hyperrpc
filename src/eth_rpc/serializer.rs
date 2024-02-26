use crate::bytes_builder::BytesBuilder;

use super::types::{BlockVariant, RpcResponse, RpcResponseData};
use bytes::Bytes;
use rayon::prelude::*;
use skar_format::Log;
use skar_format::{BlockHeader, Hex, Transaction, TransactionReceipt};

pub fn parallel_serialize(responses: Vec<RpcResponse>, batch_flag: bool) -> BytesBuilder {
    let mut builder = BytesBuilder::new();

    if responses.len() > 1 {
        let responses = responses
            .par_iter()
            .map(|resp| {
                let mut b = BytesBuilder::new();
                serialize_individual_response(&mut b, resp);
                b
            })
            .collect::<Vec<_>>();

        builder.push_static("[");
        let mut start = "";
        for resp in responses.into_iter() {
            builder.push_static(start);
            builder.extend(resp.into_iter());
            start = ",";
        }
        builder.push_static("]");
    } else if batch_flag {
        builder.push_static("[");
        serialize_individual_response(&mut builder, responses.first().unwrap());
        builder.push_static("]");
    } else {
        serialize_individual_response(&mut builder, responses.first().unwrap());
    }

    builder
}

pub fn serialize_individual_response(builder: &mut BytesBuilder, response: &RpcResponse) {
    match &response.result {
        Ok(data) => {
            builder.push_static(r#"{"id":"#);
            builder.push(Bytes::from(response.id.to_string()));
            builder.push(Bytes::from(format!(
                r#","jsonrpc":"{}","result":"#,
                response.jsonrpc
            )));

            match data {
                RpcResponseData::Block(block_variant) => match block_variant {
                    Some(block) => serialize_block(builder, block),
                    None => builder.push(Bytes::from("null")),
                },
                RpcResponseData::Logs(logs) => match logs {
                    Some(logs) => {
                        let logs = logs
                            .par_iter()
                            .map(serialize_log)
                            .map(Bytes::from)
                            .collect::<Vec<Bytes>>();
                        builder.push_json_list(logs.into_iter());
                    }
                    None => builder.push_static("[]"),
                },
                RpcResponseData::Receipts(receipts) => match receipts {
                    Some(receipts) => {
                        let receipts = receipts
                            .par_iter()
                            .map(|receipt| {
                                let mut b = BytesBuilder::new();
                                serialize_receipt(&mut b, receipt);
                                b
                            })
                            .collect::<Vec<BytesBuilder>>();

                        builder.push_static("[");
                        let mut start = "";
                        for receipt in receipts.into_iter() {
                            builder.push_static(start);
                            builder.extend(receipt.into_iter());
                            start = ",";
                        }
                        builder.push_static("]");
                    }
                    None => builder.push_static("[]"),
                },
                RpcResponseData::SingleReceipt(receipt) => match receipt {
                    Some(receipt) => serialize_receipt(builder, receipt),
                    None => builder.push_static("null"),
                },
                RpcResponseData::BlockNumber(block_number) => match block_number {
                    Some(block_number) => {
                        builder.push(Bytes::from(block_number.encode_hex_with_quotes()))
                    }
                    None => builder.push_static("null"),
                },
                RpcResponseData::Transaction(tx) => match tx {
                    Some(tx) => builder.push(Bytes::from(serialize_transaction(tx))),
                    None => builder.push_static("null"),
                },
                RpcResponseData::UninstallFilter(filter_uninstalled) => {
                    builder.push(Bytes::from(filter_uninstalled.to_string()));
                }
                RpcResponseData::Proxy(res) => {
                    // TODO: maybe handle error instead of unwrap
                    builder.push(Bytes::from(serde_json::to_vec(&res).unwrap()));
                }
            }

            builder.push_static(r#"}"#);
        }
        Err(rpc_error) => {
            builder.push(Bytes::from(format!(
                r#"{{"id":{},"jsonrpc":"2.0","error":{{"code":{},"message":{}}}}}"#,
                response.id,
                rpc_error.code,
                // do this so we have proper json escaping
                serde_json::to_string(&rpc_error.message).unwrap(),
            )));
        }
    }
}

trait BlockSerializeHelper {
    fn header(&self) -> &BlockHeader;
    fn serialize_transactions(&self) -> Vec<Bytes>;
}

impl BlockSerializeHelper for BlockVariant {
    fn header(&self) -> &BlockHeader {
        match self {
            BlockVariant::Headers(block) => &block.header,
            BlockVariant::Transactions(block) => &block.header,
        }
    }
    fn serialize_transactions(&self) -> Vec<Bytes> {
        match self {
            BlockVariant::Headers(block) => block
                .transactions
                .iter()
                .map(|tx_hash| Bytes::from(tx_hash.encode_hex_with_quotes()))
                .collect::<Vec<_>>(),
            BlockVariant::Transactions(block) => block
                .transactions
                .par_iter()
                .map(serialize_transaction)
                .map(Bytes::from)
                .collect::<Vec<_>>(),
        }
    }
}

fn serialize_block(builder: &mut BytesBuilder, block: &BlockVariant) {
    let uncles_serialized = match block.header().uncles.as_ref() {
        Some(uncles) => {
            let s = uncles
                .iter()
                .map(|uncle| uncle.encode_hex_with_quotes())
                .collect::<Vec<_>>()
                .join(",");
            format!(r#","uncles":[{}]"#, s)
        }
        None => String::new(),
    };

    let header = block.header();

    let s = format!(
        r#"{{"difficulty":{},"extraData":{},"gasLimit":{},"gasUsed":{},"hash":{},"logsBloom":{},"miner":{},"nonce":{},"number":{},"parentHash":{},"receiptsRoot":{},"sha3Uncles":{},"size":{},"stateRoot":{},"timestamp":{},"totalDifficulty":{},"transactionsRoot":{},"mixHash":{}{},"transactions":"#,
        hex_encode_opt(&header.difficulty),
        header.extra_data.encode_hex_with_quotes(),
        header.gas_limit.encode_hex_with_quotes(),
        header.gas_used.encode_hex_with_quotes(),
        header.hash.encode_hex_with_quotes(),
        header.logs_bloom.encode_hex_with_quotes(),
        header.miner.encode_hex_with_quotes(),
        hex_encode_opt(&header.nonce),
        header.number.encode_hex_with_quotes(),
        header.parent_hash.encode_hex_with_quotes(),
        header.receipts_root.encode_hex_with_quotes(),
        header.sha3_uncles.encode_hex_with_quotes(),
        header.size.encode_hex_with_quotes(),
        header.state_root.encode_hex_with_quotes(),
        header.timestamp.encode_hex_with_quotes(),
        hex_encode_opt(&header.total_difficulty),
        header.transactions_root.encode_hex_with_quotes(),
        // send empty mixHash in order to make ponder work
        // https://github.com/0xOlias/ponder/blob/df46c1bfb936e8b76ff73c08c76d8836070970de/packages/core/src/types/block.ts#L26
        skar_format::Hash::default().encode_hex_with_quotes(),
        uncles_serialized
    );
    builder.push(Bytes::from(s));
    builder.push_json_list(block.serialize_transactions().into_iter());
    builder.push_static("}");
}

fn serialize_transaction(transaction: &Transaction) -> String {
    format!(
        r#"{{"blockHash":{},"blockNumber":{},"from":{},"gas":{},"gasPrice":{},"hash":{},"input":{},"nonce":{},"to":{},"transactionIndex":{},"value":{},"chainId":{},"v":{},"r":{},"s":{},"maxPriorityFeePerGas":{},"maxFeePerGas":{}}}"#,
        transaction.block_hash.encode_hex_with_quotes(),
        transaction.block_number.encode_hex_with_quotes(),
        hex_encode_opt(&transaction.from),
        transaction.gas.encode_hex_with_quotes(),
        hex_encode_opt(&transaction.gas_price),
        transaction.hash.encode_hex_with_quotes(),
        transaction.input.encode_hex_with_quotes(),
        transaction.nonce.encode_hex_with_quotes(),
        hex_encode_opt(&transaction.to),
        transaction.transaction_index.encode_hex_with_quotes(),
        transaction.value.encode_hex_with_quotes(),
        hex_encode_opt(&transaction.chain_id),
        hex_encode_opt(&transaction.v),
        hex_encode_opt(&transaction.r),
        hex_encode_opt(&transaction.s),
        hex_encode_opt(&transaction.max_priority_fee_per_gas),
        hex_encode_opt(&transaction.max_fee_per_gas),
    )
}

fn serialize_log(log: &Log) -> String {
    format!(
        r#"{{"address":{},"topics":[{}],"data":{},"blockNumber":{},"transactionHash":{},"transactionIndex":{},"blockHash":{},"logIndex":{},"removed":{}}}"#,
        log.address.encode_hex_with_quotes(),
        log.topics
            .iter()
            .map(|t| t.encode_hex_with_quotes())
            .collect::<Vec<_>>()
            .join(","),
        log.data.encode_hex_with_quotes(),
        log.block_number.encode_hex_with_quotes(),
        log.transaction_hash.encode_hex_with_quotes(),
        log.transaction_index.encode_hex_with_quotes(),
        log.block_hash.encode_hex_with_quotes(),
        log.log_index.encode_hex_with_quotes(),
        match log.removed {
            Some(true) => "true",
            Some(false) => "false",
            None => "null",
        }
    )
}

fn serialize_receipt(builder: &mut BytesBuilder, receipt: &TransactionReceipt) {
    builder.push(Bytes::from(format!(
        r#"{{"blockHash":{},"blockNumber":{},"contractAddress":{},"cumulativeGasUsed":{},"effectiveGasPrice":{},"from":{},"gasUsed":{},"logsBloom":{},"status":{},"to":{},"transactionHash":{},"transactionIndex":{},"type":{},"logs":"#,
        receipt.block_hash.encode_hex_with_quotes(),
        receipt.block_number.encode_hex_with_quotes(),
        hex_encode_opt(&receipt.contract_address),
        receipt.cumulative_gas_used.encode_hex_with_quotes(),
        receipt.effective_gas_price.encode_hex_with_quotes(),
        receipt.from.encode_hex_with_quotes(),
        receipt.gas_used.encode_hex_with_quotes(),
        receipt.logs_bloom.encode_hex_with_quotes(),
        hex_encode_opt(&receipt.status),
        hex_encode_opt(&receipt.to),
        receipt.transaction_hash.encode_hex_with_quotes(),
        receipt.transaction_index.encode_hex_with_quotes(),
        hex_encode_opt(&receipt.kind),
    )));
    builder.push_json_list(
        receipt
            .logs
            .iter()
            .map(|log| Bytes::from(serialize_log(log))),
    );
    builder.push_static("}");
}

#[inline]
fn hex_encode_opt<T: Hex>(val: &Option<T>) -> String {
    val.as_ref()
        .map(|v| v.encode_hex_with_quotes())
        .unwrap_or_else(|| "null".to_owned())
}

#[cfg(test)]
mod tests {
    use arrayvec::ArrayVec;
    use skar_format::{Block, Hash, LogArgument};

    use super::*;

    #[test]
    fn test_serialize_receipt() {
        let receipt_src = TransactionReceipt {
            logs: vec![Log::default()],
            ..Default::default()
        };

        let mut builder = BytesBuilder::new();
        serialize_receipt(&mut builder, &receipt_src);

        let receipt: TransactionReceipt = serde_json::from_slice(&builder.build()).unwrap();

        assert_eq!(receipt_src, receipt);
    }

    #[test]
    fn test_serialize_log() {
        let mut topics = ArrayVec::new();
        topics.push(LogArgument::default());

        let log_src = Log {
            topics,
            ..Default::default()
        };

        let log: Log = serde_json::from_str(&serialize_log(&log_src)).unwrap();

        assert_eq!(log_src, log);
    }

    #[test]
    fn test_serialize_tx() {
        let tx_src = Transaction::default();

        let tx: Transaction = serde_json::from_str(&serialize_transaction(&tx_src)).unwrap();

        assert_eq!(tx_src, tx);
    }

    #[test]
    fn test_serialize_block_header() {
        let blk_src = Block {
            header: BlockHeader {
                uncles: Some(vec![Default::default()]),
                ..Default::default()
            },
            transactions: vec![Hash::default()],
        };

        let blk_variant = BlockVariant::Headers(Box::new(blk_src.clone()));

        let mut builder = BytesBuilder::new();
        serialize_block(&mut builder, &blk_variant);

        let blk: Block<Hash> = serde_json::from_slice(&builder.build()).unwrap();

        assert_eq!(blk_src, blk);
    }

    #[test]
    fn test_serialize_block_with_transactions() {
        let blk_src = Block {
            header: BlockHeader::default(),
            transactions: vec![Transaction::default()],
        };

        let blk_variant = BlockVariant::Transactions(Box::new(blk_src.clone()));

        let mut builder = BytesBuilder::new();
        serialize_block(&mut builder, &blk_variant);

        let blk: Block<Transaction> = serde_json::from_slice(&builder.build()).unwrap();

        assert_eq!(blk_src, blk);
    }
}
