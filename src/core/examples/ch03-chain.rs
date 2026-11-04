//! 第 3 章：鏈和竄改偵測。執行：`cargo run --example ch03-chain`

use blockchain_from_scratch::block::Block;
use blockchain_from_scratch::chain::{BlockStatus, Blockchain, check_blocks};
use blockchain_from_scratch::params::ChainParams;

fn show(title: &str, blocks: &[Block<String>], params: &ChainParams) {
    println!("\n== {title} ==");
    for (block, status) in blocks.iter().zip(check_blocks(blocks, params)) {
        let status = match status {
            BlockStatus::Valid => "✅ 正常".to_string(),
            BlockStatus::Invalid(error) => format!("❌ {error}"),
            BlockStatus::BuildsOnInvalid => "⚠️ 接在有問題的區塊後面".to_string(),
        };
        println!(
            "#{} {:<16} hash {}…  {status}",
            block.header.index,
            format!("{:?}", block.data),
            &block.hash[..12]
        );
    }
}

fn main() {
    // 難度 0：這一章還沒有工作量證明。
    let params = ChainParams::fixed_difficulty(0);
    let mut chain = Blockchain::new(params.clone());
    for (i, text) in ["Alice→Bob 10", "Bob→Carol 5", "Carol→Dave 1"]
        .iter()
        .enumerate()
    {
        let block = chain.next_block(1_700_000_000_000 + i as u64, text.to_string());
        chain
            .push(block)
            .expect("a block made by next_block always fits");
    }
    show("一條正常的鏈", chain.blocks(), &params);

    let mut tampered = chain.blocks().to_vec();
    tampered[1].data = "Alice→Bob 1000".to_string();
    show("偷改區塊 1 的內容", &tampered, &params);

    tampered[1].rehash();
    show("改完再重算區塊 1 的雜湊", &tampered, &params);

    for i in 2..tampered.len() {
        tampered[i].header.previous_hash = tampered[i - 1].hash.clone();
        tampered[i].rehash();
    }
    show("把後面的區塊也全部重算", &tampered, &params);
    println!(
        "\n全部又「正常」了！重算雜湊太便宜，擋不住改歷史。第 4 章的工作量證明就是來解決這件事的。"
    );
}
