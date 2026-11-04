//! 第 8 章：分岔和共識（雙重支付示範）。執行：`cargo run --example ch08-fork`

use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::sim::Network;
use blockchain_from_scratch::transaction::Transaction;
use blockchain_from_scratch::wallet::Wallet;

fn summary(net: &Network, bob: &str, carol: &str) {
    for (i, node) in net.nodes().iter().enumerate() {
        println!(
            "節點 {i}：高度 {}，最新 {}…，Bob {} 元，Carol {} 元",
            node.chain().latest().header.index,
            &node.chain().latest().hash[..10],
            node.ledger().balance(bob),
            node.ledger().balance(carol),
        );
    }
}

fn main() {
    let mut net = Network::new(3, ChainParams::fixed_difficulty(10), 100, 1_700_000_000_000);
    net.connect_all();
    net.mine(0);
    net.run_until_idle(1_000);
    let alice = net.node(0).wallet().clone();
    let bob = Wallet::generate().address();
    let carol = Wallet::generate().address();
    println!("Alice（節點 0 的礦工）挖到第一個區塊，有 50 元。");

    println!("\n== 網路斷成兩邊：{{0}} 和 {{1, 2}} ==");
    net.disconnect(0, 1);
    net.disconnect(0, 2);
    net.submit(0, Transaction::transfer(&alice, &bob, 50, 1))
        .expect("Alice has 50");
    net.mine(0);
    net.submit(1, Transaction::transfer(&alice, &carol, 50, 2))
        .expect("Alice has 50 on this side too");
    net.run_until_idle(1_000);
    net.mine(1);
    net.run_until_idle(1_000);
    net.mine(2);
    net.run_until_idle(1_000);
    println!("左邊：Alice 付 50 給 Bob。右邊：Alice 付 50 給 Carol，而且右邊多挖了一個區塊。");
    summary(&net, &bob, &carol);

    println!("\n== 重新接上線 ==");
    let seen = net.log().len();
    net.connect(0, 1);
    net.run_until_idle(1_000);
    for entry in &net.log()[seen..] {
        println!("節點 {}  {}", entry.node, entry.text);
    }
    summary(&net, &bob, &carol);
    println!(
        "\n右邊的累積工作量比較多，大家都換到右邊。Bob 收到的 50 元跟著左邊的區塊一起消失了。"
    );
    println!("所以收款的人通常會等交易後面再多幾個區塊（確認數）才算數。");
}
