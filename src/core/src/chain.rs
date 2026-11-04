//! 第 3 章：鏈和竄改偵測
//!
//! 每個區塊都記著前一個區塊的雜湊（`previous_hash`），一個接一個串成鏈。
//! 改了中間任何一個區塊的內容，它的雜湊就變了，下一個區塊記的 `previous_hash` 就對不上。
//! 所以只要從頭檢查一次，就知道有沒有人改過歷史。

use serde::Serialize;

use crate::block::{Block, data_hash};
use crate::params::ChainParams;
use crate::pow::{chain_work, meets_difficulty, next_difficulty};

/// 一個區塊為什麼不合法。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BlockError {
    #[error("創世區塊跟大家的不一樣")]
    BadGenesis,
    #[error("data_hash 對不上內容：內容被改過")]
    BadDataHash,
    #[error("hash 對不上標頭：標頭被改過")]
    BadHash,
    #[error("區塊編號應該是 {expected}，實際是 {actual}")]
    BadIndex { expected: u64, actual: u64 },
    #[error("previous_hash 對不上前一個區塊的 hash")]
    BadPreviousHash,
    #[error("時間比前一個區塊早")]
    BadTimestamp,
    #[error("難度應該是 {expected}，實際是 {actual}")]
    BadDifficulty { expected: u32, actual: u32 },
    #[error("hash 開頭的 0 不夠：難度 {difficulty} 要求至少 {difficulty} 個 0 bit")]
    InsufficientWork { difficulty: u32 },
}

/// 整條鏈裡第幾個區塊出了什麼問題。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("區塊 {index}：{error}")]
pub struct ChainError {
    pub index: usize,
    pub error: BlockError,
}

/// 檢查整條鏈時，每個區塊的狀態（網頁用它把區塊塗成綠色或紅色）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockStatus {
    /// 這個區塊沒問題，前面的也都沒問題。
    Valid,
    /// 這個區塊本身有問題。
    Invalid(BlockError),
    /// 這個區塊本身沒問題，但它接在有問題的區塊後面，所以也不能相信。
    BuildsOnInvalid,
}

/// 只看區塊自己：內容和標頭有沒有被改過。
pub fn check_contents<D: Serialize>(block: &Block<D>) -> Result<(), BlockError> {
    if data_hash(&block.data) != block.header.data_hash {
        return Err(BlockError::BadDataHash);
    }
    if block.header.compute_hash() != block.hash {
        return Err(BlockError::BadHash);
    }
    Ok(())
}

/// 檢查 `block` 能不能接在 `prefix`（從創世區塊到前一個區塊）後面。
///
/// `prefix: &[Block<D>]` 是借用一段區塊（slice），不會複製整條鏈。
pub fn check_next<D: Serialize>(
    prefix: &[Block<D>],
    block: &Block<D>,
    params: &ChainParams,
) -> Result<(), BlockError> {
    let previous = prefix
        .last()
        .expect("prefix always starts with the genesis block");

    check_contents(block)?;
    let expected_index = previous.header.index + 1;
    if block.header.index != expected_index {
        return Err(BlockError::BadIndex {
            expected: expected_index,
            actual: block.header.index,
        });
    }
    if block.header.previous_hash != previous.hash {
        return Err(BlockError::BadPreviousHash);
    }
    if block.header.timestamp < previous.header.timestamp {
        return Err(BlockError::BadTimestamp);
    }
    let expected_difficulty = next_difficulty(prefix, params);
    if block.header.difficulty != expected_difficulty {
        return Err(BlockError::BadDifficulty {
            expected: expected_difficulty,
            actual: block.header.difficulty,
        });
    }
    if !meets_difficulty(&block.hash, block.header.difficulty) {
        return Err(BlockError::InsufficientWork {
            difficulty: block.header.difficulty,
        });
    }
    Ok(())
}

/// 從頭檢查整條鏈，回傳每個區塊的狀態。
pub fn check_blocks<D: Serialize + Default + PartialEq>(
    blocks: &[Block<D>],
    params: &ChainParams,
) -> Vec<BlockStatus> {
    let mut statuses = Vec::with_capacity(blocks.len());
    let mut broken = false;
    for (i, block) in blocks.iter().enumerate() {
        let result = if i == 0 {
            check_genesis(block)
        } else {
            check_next(&blocks[..i], block, params)
        };
        let status = match result {
            Err(error) => BlockStatus::Invalid(error),
            Ok(()) if broken => BlockStatus::BuildsOnInvalid,
            Ok(()) => BlockStatus::Valid,
        };
        broken |= status != BlockStatus::Valid;
        statuses.push(status);
    }
    statuses
}

/// 創世區塊必須跟大家的一模一樣。
pub fn check_genesis<D: Serialize + Default + PartialEq>(
    block: &Block<D>,
) -> Result<(), BlockError> {
    check_contents(block)?;
    if *block != Block::genesis(D::default()) {
        return Err(BlockError::BadGenesis);
    }
    Ok(())
}

/// 一條區塊鏈：一串從創世區塊開始、彼此接得上的區塊，加上這個網路的規則參數。
#[derive(Clone, Debug)]
pub struct Blockchain<D> {
    blocks: Vec<Block<D>>,
    params: ChainParams,
}

impl<D: Serialize + Default + Clone + PartialEq> Blockchain<D> {
    /// 只有創世區塊的新鏈。
    pub fn new(params: ChainParams) -> Self {
        Blockchain {
            blocks: vec![Block::genesis(D::default())],
            params,
        }
    }

    /// 用別人給的一串區塊建一條鏈；每個區塊都會檢查，有問題就回傳是哪一個。
    pub fn from_blocks(blocks: Vec<Block<D>>, params: ChainParams) -> Result<Self, ChainError> {
        let Some(genesis) = blocks.first() else {
            return Err(ChainError {
                index: 0,
                error: BlockError::BadGenesis,
            });
        };
        check_genesis(genesis).map_err(|error| ChainError { index: 0, error })?;
        for i in 1..blocks.len() {
            check_next(&blocks[..i], &blocks[i], &params)
                .map_err(|error| ChainError { index: i, error })?;
        }
        Ok(Blockchain { blocks, params })
    }

    pub fn blocks(&self) -> &[Block<D>] {
        &self.blocks
    }

    pub fn params(&self) -> &ChainParams {
        &self.params
    }

    /// 最新的區塊。鏈至少有創世區塊，所以一定有。
    pub fn latest(&self) -> &Block<D> {
        self.blocks
            .last()
            .expect("a chain always has the genesis block")
    }

    /// 有幾個區塊（含創世區塊）。
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// 鏈至少有創世區塊，所以永遠不是空的；這個函式是給 clippy 看的慣例。
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// 下一個區塊該用的難度（第 4 章）。
    pub fn next_difficulty(&self) -> u32 {
        next_difficulty(&self.blocks, &self.params)
    }

    /// 累積工作量：每個區塊平均要試幾次雜湊，全部加起來（第 8 章）。
    pub fn total_work(&self) -> u128 {
        chain_work(&self.blocks)
    }

    /// 接在最新區塊後面的新區塊（還沒挖礦）。
    ///
    /// 時鐘不準的時候，時間至少不要比前一個區塊早，不然區塊會不合法。
    pub fn next_block(&self, timestamp: u64, data: D) -> Block<D> {
        let latest = self.latest();
        Block::new(
            latest.header.index + 1,
            latest.hash.clone(),
            timestamp.max(latest.header.timestamp),
            self.next_difficulty(),
            data,
        )
    }

    /// 檢查後把區塊接到鏈的最後面。
    pub fn push(&mut self, block: Block<D>) -> Result<(), BlockError> {
        check_next(&self.blocks, &block, &self.params)?;
        self.blocks.push(block);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_chain(texts: &[&str]) -> Blockchain<String> {
        let mut chain = Blockchain::new(ChainParams::fixed_difficulty(0));
        for (i, text) in texts.iter().enumerate() {
            let block = chain.next_block(1_700_000_000_000 + i as u64, text.to_string());
            chain.push(block).unwrap();
        }
        chain
    }

    #[test]
    fn a_fresh_chain_is_valid() {
        let chain = text_chain(&["a", "b", "c"]);
        assert_eq!(chain.len(), 4);
        let statuses = check_blocks(chain.blocks(), chain.params());
        assert!(statuses.iter().all(|s| *s == BlockStatus::Valid));
    }

    #[test]
    fn tampered_data_is_detected_and_everything_after_is_untrusted() {
        let chain = text_chain(&["a", "b", "c"]);
        let mut blocks = chain.blocks().to_vec();
        blocks[1].data = "A".to_string();

        let statuses = check_blocks(&blocks, chain.params());
        assert_eq!(statuses[0], BlockStatus::Valid);
        assert_eq!(statuses[1], BlockStatus::Invalid(BlockError::BadDataHash));
        assert_eq!(statuses[2], BlockStatus::BuildsOnInvalid);
        assert_eq!(statuses[3], BlockStatus::BuildsOnInvalid);
    }

    #[test]
    fn rehashing_a_tampered_block_breaks_the_next_link() {
        let chain = text_chain(&["a", "b", "c"]);
        let mut blocks = chain.blocks().to_vec();
        blocks[1].data = "A".to_string();
        blocks[1].rehash();

        let statuses = check_blocks(&blocks, chain.params());
        assert_eq!(statuses[1], BlockStatus::Valid);
        assert_eq!(
            statuses[2],
            BlockStatus::Invalid(BlockError::BadPreviousHash)
        );
    }

    #[test]
    fn from_blocks_reports_the_first_bad_block() {
        let chain = text_chain(&["a", "b"]);
        let mut blocks = chain.blocks().to_vec();
        blocks[2].header.timestamp = 0;
        blocks[2].rehash();
        let err = Blockchain::from_blocks(blocks, chain.params().clone()).unwrap_err();
        assert_eq!(err.index, 2);
        assert_eq!(err.error, BlockError::BadTimestamp);
    }

    #[test]
    fn rejects_a_different_genesis() {
        let mut blocks = text_chain(&[]).blocks().to_vec();
        blocks[0].data = "my genesis block!!".to_string();
        blocks[0].rehash();
        let err = Blockchain::from_blocks(blocks, ChainParams::fixed_difficulty(0)).unwrap_err();
        assert_eq!(err.error, BlockError::BadGenesis);
    }

    #[test]
    fn rejects_wrong_index() {
        let chain = text_chain(&["a"]);
        let mut block = chain.next_block(1_800_000_000_000, "b".to_string());
        block.header.index = 5;
        block.rehash();
        let mut chain = chain;
        assert_eq!(
            chain.push(block),
            Err(BlockError::BadIndex {
                expected: 2,
                actual: 5
            })
        );
    }
}
