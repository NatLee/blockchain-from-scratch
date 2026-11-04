//! 網路參數：同一個網路裡的節點必須用同一組參數，不然彼此會認為對方的區塊不合法。

use serde::{Deserialize, Serialize};

/// 鏈的規則參數。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChainParams {
    /// 第 1 個區塊的難度（雜湊開頭至少幾個 0 bit）。
    pub initial_difficulty: u32,
    /// 難度往下調時的下限。
    pub min_difficulty: u32,
    /// 難度往上調時的上限。
    pub max_difficulty: u32,
    /// 希望多久出一個區塊，毫秒。
    pub target_block_time_ms: u64,
    /// 每幾個區塊調整一次難度。
    pub adjustment_interval: u64,
    /// 出塊獎勵：礦工每挖到一個區塊，可以給自己多少錢（第 6 章）。
    pub block_reward: u64,
}

impl Default for ChainParams {
    /// 節點預設的參數。難度 16 大約要試 6 萬 5 千次雜湊，一般電腦不到一秒。
    fn default() -> Self {
        ChainParams {
            initial_difficulty: 16,
            min_difficulty: 8,
            max_difficulty: 32,
            target_block_time_ms: 10_000,
            adjustment_interval: 10,
            block_reward: 50,
        }
    }
}

impl ChainParams {
    /// 難度固定不變的參數。第 2、3 章用難度 0（不用挖礦），測試用小難度跑得快。
    pub fn fixed_difficulty(difficulty: u32) -> Self {
        ChainParams {
            initial_difficulty: difficulty,
            min_difficulty: difficulty,
            max_difficulty: difficulty,
            ..ChainParams::default()
        }
    }
}
