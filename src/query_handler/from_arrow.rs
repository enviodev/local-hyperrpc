use std::collections::BTreeMap;

use anyhow::{Context, Result};

use arrayvec::ArrayVec;
use arrow2::array::{BinaryArray, BooleanArray, UInt64Array, UInt8Array};
use skar_client::ArrowBatch;
use skar_format::{
    Block, BlockHeader, Hash, Log, Transaction, TransactionReceipt, TransactionStatus,
    TransactionType,
};

pub fn batch_to_block_headers<Tx>(
    batch: ArrowBatch,
    blocks: &mut BTreeMap<u64, Block<Tx>>,
) -> Result<()> {
    let number = batch
        .column::<UInt64Array>("number")
        .context("get column")?;
    let hash = batch
        .column::<BinaryArray<i32>>("hash")
        .context("get column")?;
    let parent_hash = batch
        .column::<BinaryArray<i32>>("parent_hash")
        .context("get column")?;
    let nonce = batch
        .column::<BinaryArray<i32>>("nonce")
        .context("get column")?;
    let sha3_uncles = batch
        .column::<BinaryArray<i32>>("sha3_uncles")
        .context("get column")?;
    let logs_bloom = batch
        .column::<BinaryArray<i32>>("logs_bloom")
        .context("get column")?;
    let transactions_root = batch
        .column::<BinaryArray<i32>>("transactions_root")
        .context("get column")?;
    let state_root = batch
        .column::<BinaryArray<i32>>("state_root")
        .context("get column")?;
    let receipts_root = batch
        .column::<BinaryArray<i32>>("receipts_root")
        .context("get column")?;
    let miner = batch
        .column::<BinaryArray<i32>>("miner")
        .context("get column")?;
    let difficulty = batch
        .column::<BinaryArray<i32>>("difficulty")
        .context("get column")?;
    let total_difficulty = batch
        .column::<BinaryArray<i32>>("total_difficulty")
        .context("get column")?;
    let extra_data = batch
        .column::<BinaryArray<i32>>("extra_data")
        .context("get column")?;
    let size = batch
        .column::<BinaryArray<i32>>("size")
        .context("get column")?;
    let gas_limit = batch
        .column::<BinaryArray<i32>>("gas_limit")
        .context("get column")?;
    let gas_used = batch
        .column::<BinaryArray<i32>>("gas_used")
        .context("get column")?;
    let timestamp = batch
        .column::<BinaryArray<i32>>("timestamp")
        .context("get column")?;
    let uncles = batch
        .column::<BinaryArray<i32>>("uncles")
        .context("get column")?;
    let base_fee_per_gas = batch
        .column::<BinaryArray<i32>>("base_fee_per_gas")
        .context("get column")?;

    for i in 0..number.len() {
        let block = Block {
            header: BlockHeader {
                number: number.value(i).into(),
                hash: hash.value(i).try_into().unwrap(),
                parent_hash: parent_hash.value(i).try_into().unwrap(),
                nonce: nonce.get(i).map(|b| b.try_into().unwrap()),
                sha3_uncles: sha3_uncles.value(i).try_into().unwrap(),
                logs_bloom: logs_bloom.value(i).into(),
                transactions_root: transactions_root.value(i).try_into().unwrap(),
                state_root: state_root.value(i).try_into().unwrap(),
                receipts_root: receipts_root.value(i).try_into().unwrap(),
                miner: miner.value(i).try_into().unwrap(),
                difficulty: difficulty.get(i).map(|b| b.into()),
                total_difficulty: total_difficulty.get(i).map(|b| b.into()),
                extra_data: extra_data.value(i).into(),
                size: size.value(i).into(),
                gas_limit: gas_limit.value(i).into(),
                gas_used: gas_used.value(i).into(),
                timestamp: timestamp.value(i).into(),
                uncles: {
                    let val: Vec<Hash> = uncles
                        .value(i)
                        .chunks(32)
                        .map(|b| b.try_into().unwrap())
                        .collect();

                    if val.is_empty() {
                        None
                    } else {
                        Some(val)
                    }
                },
                base_fee_per_gas: base_fee_per_gas.get(i).map(|b| b.into()),
            },
            transactions: Vec::new(),
        };

        blocks.insert(block.header.number.into(), block);
    }

    Ok(())
}

pub fn batch_to_receipts(
    batch: ArrowBatch,
    receipts: &mut BTreeMap<(u64, u64), TransactionReceipt>,
) -> Result<()> {
    let transaction_hash = batch
        .column::<BinaryArray<i32>>("hash")
        .context("get column")?;
    let transaction_index = batch
        .column::<UInt64Array>("transaction_index")
        .context("get column")?;
    let block_hash = batch
        .column::<BinaryArray<i32>>("block_hash")
        .context("get column")?;
    let block_number = batch
        .column::<UInt64Array>("block_number")
        .context("get column")?;
    let from = batch
        .column::<BinaryArray<i32>>("from")
        .context("get column")?;
    let to = batch
        .column::<BinaryArray<i32>>("to")
        .context("get column")?;
    let cumulative_gas_used = batch
        .column::<BinaryArray<i32>>("cumulative_gas_used")
        .context("get column")?;
    let effective_gas_price = batch
        .column::<BinaryArray<i32>>("effective_gas_price")
        .context("get column")?;
    let gas_used = batch
        .column::<BinaryArray<i32>>("gas_used")
        .context("get column")?;
    let contract_address = batch
        .column::<BinaryArray<i32>>("contract_address")
        .context("get column")?;
    let logs_bloom = batch
        .column::<BinaryArray<i32>>("logs_bloom")
        .context("get column")?;
    let kind = batch.column::<UInt8Array>("type").context("get column")?;
    let root = batch
        .column::<BinaryArray<i32>>("root")
        .context("get column")?;
    let status = batch.column::<UInt8Array>("status").context("get column")?;

    for i in 0..status.len() {
        let receipt = TransactionReceipt {
            transaction_hash: transaction_hash.value(i).try_into().unwrap(),
            transaction_index: transaction_index.value(i).into(),
            block_hash: block_hash.value(i).try_into().unwrap(),
            block_number: block_number.value(i).into(),
            from: from.value(i).try_into().unwrap(),
            to: to.get(i).map(|b| b.try_into().unwrap()),
            cumulative_gas_used: cumulative_gas_used.value(i).into(),
            effective_gas_price: effective_gas_price.value(i).into(),
            gas_used: gas_used.value(i).into(),
            contract_address: contract_address.get(i).map(|b| b.try_into().unwrap()),
            logs_bloom: logs_bloom.value(i).into(),
            kind: kind.get(i).map(TransactionType::from),
            root: root.get(i).map(|b| b.try_into().unwrap()),
            status: status
                .get(i)
                .map(|b| TransactionStatus::from_u8(b).unwrap()),
            logs: Vec::new(),
        };

        receipts.insert(
            (
                receipt.block_number.into(),
                receipt.transaction_index.into(),
            ),
            receipt,
        );
    }

    Ok(())
}

pub fn batch_to_logs(batch: &ArrowBatch) -> Result<impl Iterator<Item = Log> + '_> {
    let removed = batch
        .column::<BooleanArray>("removed")
        .context("get column")?;
    let log_index = batch
        .column::<UInt64Array>("log_index")
        .context("get column")?;
    let transaction_index = batch
        .column::<UInt64Array>("transaction_index")
        .context("get column")?;
    let transaction_hash = batch
        .column::<BinaryArray<i32>>("transaction_hash")
        .context("get column")?;
    let block_hash = batch
        .column::<BinaryArray<i32>>("block_hash")
        .context("get column")?;
    let block_number = batch
        .column::<UInt64Array>("block_number")
        .context("get column")?;
    let address = batch
        .column::<BinaryArray<i32>>("address")
        .context("get column")?;
    let data = batch
        .column::<BinaryArray<i32>>("data")
        .context("get column")?;
    let topic0 = batch
        .column::<BinaryArray<i32>>("topic0")
        .context("get column")?;
    let topic1 = batch
        .column::<BinaryArray<i32>>("topic1")
        .context("get column")?;
    let topic2 = batch
        .column::<BinaryArray<i32>>("topic2")
        .context("get column")?;
    let topic3 = batch
        .column::<BinaryArray<i32>>("topic3")
        .context("get column")?;

    Ok((0..removed.len()).map(|i| {
        let mut topics = ArrayVec::new();
        if let Some(topic) = topic0.get(i) {
            topics.push(topic.try_into().unwrap());
        }
        if let Some(topic) = topic1.get(i) {
            topics.push(topic.try_into().unwrap());
        }
        if let Some(topic) = topic2.get(i) {
            topics.push(topic.try_into().unwrap());
        }
        if let Some(topic) = topic3.get(i) {
            topics.push(topic.try_into().unwrap());
        }

        Log {
            removed: removed.get(i),
            log_index: log_index.value(i).into(),
            transaction_index: transaction_index.value(i).into(),
            transaction_hash: transaction_hash.value(i).try_into().unwrap(),
            block_hash: block_hash.value(i).try_into().unwrap(),
            block_number: block_number.value(i).into(),
            address: address.value(i).try_into().unwrap(),
            data: data.value(i).into(),
            topics,
        }
    }))
}

pub fn batch_to_transactions(
    batch: ArrowBatch,
    blocks: &mut BTreeMap<u64, Block<Transaction>>,
) -> Result<()> {
    let block_hash = batch
        .column::<BinaryArray<i32>>("block_hash")
        .context("get column")?;
    let block_number = batch
        .column::<UInt64Array>("block_number")
        .context("get column")?;
    let from = batch
        .column::<BinaryArray<i32>>("from")
        .context("get column")?;
    let gas = batch
        .column::<BinaryArray<i32>>("gas")
        .context("get column")?;
    let gas_price = batch
        .column::<BinaryArray<i32>>("gas_price")
        .context("get column")?;
    let hash = batch
        .column::<BinaryArray<i32>>("hash")
        .context("get column")?;
    let input = batch
        .column::<BinaryArray<i32>>("input")
        .context("get column")?;
    let nonce = batch
        .column::<BinaryArray<i32>>("nonce")
        .context("get column")?;
    let to = batch
        .column::<BinaryArray<i32>>("to")
        .context("get column")?;
    let transaction_index = batch
        .column::<UInt64Array>("transaction_index")
        .context("get column")?;
    let value = batch
        .column::<BinaryArray<i32>>("value")
        .context("get column")?;
    let v = batch
        .column::<BinaryArray<i32>>("v")
        .context("get column")?;
    let r = batch
        .column::<BinaryArray<i32>>("r")
        .context("get column")?;
    let s = batch
        .column::<BinaryArray<i32>>("s")
        .context("get column")?;
    let max_priority_fee_per_gas = batch
        .column::<BinaryArray<i32>>("max_priority_fee_per_gas")
        .context("get column")?;
    let max_fee_per_gas = batch
        .column::<BinaryArray<i32>>("max_fee_per_gas")
        .context("get column")?;
    let chain_id = batch
        .column::<BinaryArray<i32>>("chain_id")
        .context("get column")?;

    for i in 0..block_hash.len() {
        let tx = Transaction {
            block_hash: block_hash.value(i).try_into().unwrap(),
            block_number: block_number.value(i).into(),
            from: from.get(i).map(|b| b.try_into().unwrap()),
            gas: gas.value(i).into(),
            gas_price: gas_price.get(i).map(|b| b.into()),
            hash: hash.value(i).try_into().unwrap(),
            input: input.value(i).into(),
            nonce: nonce.value(i).into(),
            to: to.get(i).map(|b| b.try_into().unwrap()),
            transaction_index: transaction_index.value(i).into(),
            value: value.value(i).into(),
            v: v.get(i).map(|b| b.into()),
            r: r.get(i).map(|b| b.into()),
            s: s.get(i).map(|b| b.into()),
            max_priority_fee_per_gas: max_priority_fee_per_gas.get(i).map(|b| b.into()),
            max_fee_per_gas: max_fee_per_gas.get(i).map(|b| b.into()),
            chain_id: chain_id.get(i).map(|b| b.into()),
        };

        blocks
            .get_mut(&tx.block_number)
            .context("get block num")?
            .transactions
            .push(tx);
    }

    Ok(())
}
