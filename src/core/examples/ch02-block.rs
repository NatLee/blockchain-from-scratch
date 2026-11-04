//! 第 2 章：區塊。執行：`cargo run --example ch02-block`

use blockchain_from_scratch::block::Block;

fn main() {
    let genesis: Block<String> = Block::genesis(String::new());
    let block = Block::new(
        1,
        genesis.hash.clone(),
        1_700_000_000_000,
        0,
        "Alice 付給 Bob 10 元".to_string(),
    );

    println!("== 區塊的雜湊只算標頭 ==");
    println!("要雜湊的字串：{}", block.header.hash_input());
    println!("區塊雜湊：    {}", block.hash);

    println!("\n== 內容改一個字，data_hash 和區塊雜湊都會變 ==");
    let mut changed = block.clone();
    changed.data = "Alice 付給 Bob 99 元".to_string();
    changed.rehash();
    println!("原本 data_hash：{}", block.header.data_hash);
    println!("改後 data_hash：{}", changed.header.data_hash);
    println!("原本 hash：     {}", block.hash);
    println!("改後 hash：     {}", changed.hash);

    println!("\n== 區塊轉成 JSON（節點之間傳的就是這個）==");
    println!(
        "{}",
        serde_json::to_string_pretty(&block).expect("blocks serialize to JSON")
    );
}
