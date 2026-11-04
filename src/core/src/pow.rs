//! 第 4 章：工作量證明（proof of work）和難度
//!
//! 第 3 章的鏈有個漏洞：改了一個區塊之後，只要把後面每個區塊的雜湊都重算一次，鏈又「合法」了。
//! 電腦重算雜湊只要幾微秒，所以這擋不住任何人。
//!
//! 工作量證明的想法：規定區塊的雜湊開頭必須有好幾個 0。雜湊無法預測，唯一的辦法是一直換
//! `nonce`、一直算，直到碰巧算出符合的雜湊。難度 `d` 平均要試 `2^d` 次。
//! 想改歷史，就得把被改的區塊和後面所有區塊重新挖一遍，而且要比全網路其他人挖得還快。

use crate::block::{Block, Header};
use crate::hash::{decode_hash_hex, leading_zero_bits, sha256};
use crate::params::ChainParams;

/// 雜湊有沒有達到難度：開頭至少要有 `difficulty` 個 0 bit。
pub fn meets_difficulty(hash_hex: &str, difficulty: u32) -> bool {
    match decode_hash_hex(hash_hex) {
        Some(bytes) => leading_zero_bits(&bytes) >= difficulty,
        None => false,
    }
}

/// 從 `header.nonce` 開始往上試，最多試 `max_attempts` 次。
///
/// 找到就回傳區塊雜湊，`header.nonce` 停在找到的那個數字；沒找到就回傳 `None`，
/// `header.nonce` 停在下一個要試的數字，下次可以接著試。網頁用這個函式分段挖礦，畫面才不會卡住。
pub fn try_mine(header: &mut Header, max_attempts: u64) -> Option<String> {
    for _ in 0..max_attempts {
        let hash = sha256(header.hash_input().as_bytes());
        if leading_zero_bits(&hash) >= header.difficulty {
            return Some(hex::encode(hash));
        }
        header.nonce = header.nonce.wrapping_add(1);
    }
    None
}

/// 挖到為止。回傳挖好的區塊，和總共試了幾次。
pub fn mine<D>(mut block: Block<D>) -> (Block<D>, u64) {
    let start = block.header.nonce;
    let hash = loop {
        if let Some(hash) = try_mine(&mut block.header, 1 << 16) {
            break hash;
        }
    };
    let attempts = block.header.nonce.wrapping_sub(start) + 1;
    block.hash = hash;
    (block, attempts)
}

/// 難度 `difficulty` 的區塊平均要試幾次雜湊：`2^difficulty`。
pub fn block_work(difficulty: u32) -> u128 {
    1u128 << difficulty.min(127)
}

/// 累積工作量：整條鏈每個區塊的工作量加起來。第 8 章用它決定要相信哪一條鏈。
pub fn chain_work<D>(blocks: &[Block<D>]) -> u128 {
    blocks
        .iter()
        .map(|b| block_work(b.header.difficulty))
        .fold(0u128, u128::saturating_add)
}

/// 接在 `blocks` 後面的下一個區塊，難度應該是多少。
///
/// 規則（跟 naivecoin 一樣簡單）：
/// - 第 1 個區塊用 `initial_difficulty`。
/// - 每 `adjustment_interval` 個區塊檢查一次：最近這段時間出塊太快（不到預期的一半）就加 1，
///   太慢（超過預期的兩倍）就減 1，其餘不變。
/// - 計時不含創世區塊，因為它的時間固定在 2018 年。
pub fn next_difficulty<D>(blocks: &[Block<D>], params: &ChainParams) -> u32 {
    // 只有創世區塊（或什麼都沒有）時，下一個就是第 1 個區塊。
    let Some(latest) = blocks.last().filter(|_| blocks.len() > 1) else {
        return params.initial_difficulty;
    };
    // 下一個區塊的編號 = 現在有幾個區塊。
    let next_index = blocks.len();
    let interval = params.adjustment_interval.max(1) as usize;
    if !next_index.is_multiple_of(interval) || blocks.len() < interval + 2 {
        return latest.header.difficulty;
    }

    // `interval` 個區塊之前的那一個；`blocks.len() >= interval + 2` 保證它不是創世區塊。
    let window_start = &blocks[blocks.len() - 1 - interval];
    let taken = latest
        .header
        .timestamp
        .saturating_sub(window_start.header.timestamp);
    let expected = interval as u64 * params.target_block_time_ms;
    let current = latest.header.difficulty;
    let adjusted = if taken < expected / 2 {
        current.saturating_add(1)
    } else if taken > expected * 2 {
        current.saturating_sub(1)
    } else {
        current
    };
    adjusted.clamp(params.min_difficulty, params.max_difficulty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Blockchain;

    #[test]
    fn mined_block_meets_its_difficulty() {
        let block = Block::new(1, "00".repeat(32), 1_700_000_000_000, 10, "x".to_string());
        let (mined, attempts) = mine(block);
        assert!(meets_difficulty(&mined.hash, 10));
        assert_eq!(mined.hash, mined.header.compute_hash());
        assert!(attempts >= 1);
    }

    #[test]
    fn try_mine_can_resume() {
        let mut header =
            Block::new(1, "00".repeat(32), 1_700_000_000_000, 12, "x".to_string()).header;
        let mut tries = 0;
        let hash = loop {
            tries += 1;
            if let Some(hash) = try_mine(&mut header, 100) {
                break hash;
            }
        };
        assert!(tries >= 1);
        assert!(meets_difficulty(&hash, 12));
        assert_eq!(header.compute_hash(), hash);
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(!meets_difficulty("zz", 0));
    }

    fn params() -> ChainParams {
        ChainParams {
            initial_difficulty: 4,
            min_difficulty: 2,
            max_difficulty: 8,
            target_block_time_ms: 1_000,
            adjustment_interval: 5,
            block_reward: 50,
        }
    }

    /// 用固定的出塊間隔挖 `count` 個區塊，回傳每個區塊的難度。
    fn difficulties_with_gap(gap_ms: u64, count: usize) -> Vec<u32> {
        let mut chain: Blockchain<String> = Blockchain::new(params());
        let mut time = 1_700_000_000_000;
        for _ in 0..count {
            time += gap_ms;
            let (block, _) = mine(chain.next_block(time, String::new()));
            chain.push(block).unwrap();
        }
        chain.blocks()[1..]
            .iter()
            .map(|b| b.header.difficulty)
            .collect()
    }

    #[test]
    fn fast_blocks_raise_difficulty() {
        let d = difficulties_with_gap(100, 15);
        assert_eq!(d[0], 4);
        assert_eq!(*d.last().unwrap(), 6, "{d:?}");
    }

    #[test]
    fn slow_blocks_lower_difficulty_but_not_below_min() {
        let d = difficulties_with_gap(10_000, 20);
        assert_eq!(*d.last().unwrap(), 2, "{d:?}");
    }

    #[test]
    fn on_target_blocks_keep_difficulty() {
        let d = difficulties_with_gap(1_000, 15);
        assert!(d.iter().all(|x| *x == 4), "{d:?}");
    }

    #[test]
    fn work_doubles_with_each_difficulty_step() {
        assert_eq!(block_work(0), 1);
        assert_eq!(block_work(10), 1024);
        assert_eq!(block_work(11), 2 * block_work(10));
    }
}
