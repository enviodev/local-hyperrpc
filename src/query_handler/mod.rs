use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use moka::sync::Cache;

use skar_format::{Block, Hash, Transaction, TransactionReceipt};
use skar_net_types::{FieldSelection, Query, TransactionSelection};

use crate::{
    query_handler::from_arrow::{batch_to_block_headers, batch_to_transactions},
    BlockRange,
};

use self::from_arrow::{batch_to_logs, batch_to_receipts};

pub mod from_arrow;

#[derive(Clone)]
pub struct QueryHandler {
    client: skar_client::Client,
    blocks_cache: Cache<u64, Block<Hash>>,
    blocks_with_txs_cache: Cache<u64, Block<Transaction>>,
    read_ahead: u64,
}

impl QueryHandler {
    pub fn new(client: skar_client::Client, read_ahead: u64) -> Self {
        Self {
            client,
            blocks_with_txs_cache: Cache::new(100_000),
            blocks_cache: Cache::new(100_000),
            read_ahead,
        }
    }

    pub async fn get_blocks(&self, block_range: BlockRange) -> Result<BTreeMap<u64, Block<Hash>>> {
        let mut blocks = BTreeMap::new();
        let mut block_num = block_range.0;

        while block_num < block_range.1 {
            match self.blocks_cache.get(&block_num) {
                Some(block) => {
                    blocks.insert(block_num, block);
                }
                None => break,
            }

            block_num += 1;
        }

        if block_num == block_range.1 {
            return Ok(blocks);
        }

        let height = self.client.get_height().await.context("get height")?;
        let req_range = BlockRange(
            block_num,
            std::cmp::min(
                height + 1,
                std::cmp::max(block_range.1, block_range.0 + self.read_ahead),
            ),
        );

        let res = self
            .client
            .send::<skar_client::ArrowIpc>(&Query {
                from_block: req_range.0,
                to_block: Some(req_range.1),
                include_all_blocks: true,
                field_selection: FieldSelection {
                    block: skar_schema::block_header()
                        .fields
                        .iter()
                        .map(|f| f.name.clone())
                        .collect(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .context("run skar query")?;

        if res.next_block != req_range.1 {
            return Err(anyhow!("Request took too long to handle"));
        }

        for batch in res.data.blocks {
            batch_to_block_headers(batch, &mut blocks).context("batch to blocks")?;
        }

        for (&k, v) in blocks.range(req_range.0..req_range.1) {
            self.blocks_cache.insert(k, v.clone());
        }

        blocks.retain(|&k, _| k >= block_range.0 && k < block_range.1);

        Ok(blocks)
    }

    pub async fn get_blocks_with_transactions(
        &self,
        block_range: BlockRange,
    ) -> Result<BTreeMap<u64, Block<Transaction>>> {
        let mut blocks = BTreeMap::new();
        let mut block_num = block_range.0;

        while block_num < block_range.1 {
            match self.blocks_with_txs_cache.get(&block_num) {
                Some(block) => {
                    blocks.insert(block_num, block);
                }
                None => break,
            }

            block_num += 1;
        }

        if block_num == block_range.1 {
            return Ok(blocks);
        }

        let height = self.client.get_height().await.context("get height")?;
        let req_range = BlockRange(
            block_num,
            std::cmp::min(
                height + 1,
                std::cmp::max(block_range.1, block_range.0 + self.read_ahead),
            ),
        );

        let res = self
            .client
            .send::<skar_client::ArrowIpc>(&Query {
                from_block: req_range.0,
                to_block: Some(req_range.1),
                include_all_blocks: true,
                transactions: vec![TransactionSelection::default()],
                field_selection: FieldSelection {
                    block: skar_schema::block_header()
                        .fields
                        .iter()
                        .map(|f| f.name.clone())
                        .collect(),
                    transaction: TX_FIELDS.iter().map(|&f| f.to_owned()).collect(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .context("run skar query")?;

        if res.next_block != req_range.1 {
            return Err(anyhow!("Request took too long to handle"));
        }

        for batch in res.data.blocks {
            batch_to_block_headers(batch, &mut blocks).context("batch to blocks")?;
        }

        for batch in res.data.transactions {
            batch_to_transactions(batch, &mut blocks).context("batch to transactions")?;
        }

        for (&k, v) in blocks.range(req_range.0..req_range.1) {
            self.blocks_with_txs_cache.insert(k, v.clone());
        }

        blocks.retain(|&k, _| k >= block_range.0 && k < block_range.1);

        Ok(blocks)
    }

    pub async fn get_block_receipts(
        &self,
        block_range: BlockRange,
    ) -> Result<BTreeMap<(u64, u64), TransactionReceipt>> {
        let res = self
            .client
            .send::<skar_client::ArrowIpc>(&Query {
                from_block: block_range.0,
                to_block: Some(block_range.1),
                include_all_blocks: true,
                transactions: vec![TransactionSelection::default()],
                field_selection: FieldSelection {
                    transaction: RECEIPT_FIELDS.iter().map(|&f| f.to_owned()).collect(),
                    log: skar_schema::log()
                        .fields
                        .iter()
                        .map(|f| f.name.clone())
                        .collect(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .context("run skar query")?;

        if res.next_block != block_range.1 {
            return Err(anyhow!("Request took too long to handle"));
        }

        let mut receipts = BTreeMap::new();

        for batch in res.data.transactions {
            batch_to_receipts(batch, &mut receipts).context("batch to receipts")?;
        }

        for batch in res.data.logs {
            for log in batch_to_logs(&batch).context("batch to logs")? {
                if let Some(tgt) =
                    receipts.get_mut(&(log.block_number.into(), log.transaction_index.into()))
                {
                    tgt.logs.push(log);
                }
            }
        }

        Ok(receipts)
    }
}

const TX_FIELDS: &[&str] = &[
    "block_hash",
    "block_number",
    "from",
    "gas",
    "gas_price",
    "hash",
    "input",
    "nonce",
    "to",
    "transaction_index",
    "value",
    "v",
    "r",
    "s",
    "max_priority_fee_per_gas",
    "max_fee_per_gas",
    "chain_id",
];

const RECEIPT_FIELDS: &[&str] = &[
    "hash",
    "transaction_index",
    "block_hash",
    "block_number",
    "from",
    "to",
    "cumulative_gas_used",
    "effective_gas_price",
    "gas_used",
    "contract_address",
    "logs_bloom",
    "type",
    "root",
    "status",
];
