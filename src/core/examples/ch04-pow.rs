//! 第 4 章：工作量證明和難度。執行：`cargo run --release --example ch04-pow`
//!
//! 加 `--release` 會快很多（編譯器會最佳化）；不加也能跑，只是難度高的時候要等比較久。

use std::time::Instant;

use blockchain_from_scratch::block::Block;
use blockchain_from_scratch::chain::Blockchain;
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::pow::mine;

fn main() {
    println!("== 難度每加 1，平均要試的次數就變兩倍 ==");
    for difficulty in [4, 8, 12, 16, 20] {
        let block = Block::new(
            1,
            "00".repeat(32),
            1_700_000_000_000,
            difficulty,
            "挖礦".to_string(),
        );
        let started = Instant::now();
        let (mined, attempts) = mine(block);
        println!(
            "難度 {difficulty:>2}：試了 {attempts:>9} 次（理論平均 {:>9}），{:>10.1?}，hash {}…",
            1u64 << difficulty,
            started.elapsed(),
            &mined.hash[..16]
        );
    }

    println!("\n== 出塊太快，難度自動往上調 ==");
    let params = ChainParams {
        initial_difficulty: 8,
        min_difficulty: 4,
        max_difficulty: 20,
        target_block_time_ms: 1_000,
        adjustment_interval: 5,
        block_reward: 50,
    };
    let mut chain: Blockchain<String> = Blockchain::new(params);
    // 模擬時間：每 100 毫秒就出一個區塊，比目標的 1 秒快 10 倍。
    let mut time = 1_700_000_000_000;
    for _ in 0..20 {
        time += 100;
        let (block, _) = mine(chain.next_block(time, String::new()));
        chain.push(block).expect("mined on our own tip");
    }
    let difficulties: Vec<u32> = chain.blocks()[1..]
        .iter()
        .map(|b| b.header.difficulty)
        .collect();
    println!("每個區塊的難度：{difficulties:?}");
    println!("累積工作量：約 {} 次雜湊", chain.total_work());
}
