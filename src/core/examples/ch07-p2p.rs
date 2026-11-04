//! 第 7 章：P2P 節點和廣播。執行：`cargo run --example ch07-p2p`
//!
//! 這裡用記憶體裡的模擬網路；要跑真的 3 個節點，見 README 的 docker compose。

use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::sim::Network;
use blockchain_from_scratch::transaction::Transaction;

/// 印出 `from` 之後的 log，回傳現在 log 有幾筆。
fn print_log(net: &Network, from: usize, start: u64) -> usize {
    for entry in &net.log()[from..] {
        println!(
            "{:>5} ms  節點 {}  {}",
            entry.time - start,
            entry.node,
            entry.text
        );
    }
    net.log().len()
}

fn main() {
    // 3 個節點排成一條線：0 — 1 — 2，訊息延遲 100 毫秒。
    let start = 1_700_000_000_000;
    let mut net = Network::new(3, ChainParams::fixed_difficulty(8), 100, start);
    net.connect(0, 1);
    net.connect(1, 2);
    net.run_until_idle(1_000);

    println!("== 接上線：節點互相問對方最新的區塊 ==");
    let seen = print_log(&net, 0, start);

    println!("\n== 節點 0 挖到一個區塊：一路傳到節點 2 ==");
    net.mine(0);
    net.run_until_idle(1_000);
    let seen = print_log(&net, seen, start);

    println!("\n== 節點 0 的礦工轉 10 元給節點 2 的礦工：交易也是一路轉傳 ==");
    let tx = Transaction::transfer(net.node(0).wallet(), &net.node(2).address(), 10, net.now());
    net.submit(0, tx).expect("node 0 has the block reward");
    net.run_until_idle(1_000);
    print_log(&net, seen, start);
    for (i, node) in net.nodes().iter().enumerate() {
        println!(
            "節點 {i}：鏈長 {}，mempool {} 筆",
            node.chain().len(),
            node.mempool().len()
        );
    }
    println!("總共送了 {} 則訊息", net.delivered());
}
