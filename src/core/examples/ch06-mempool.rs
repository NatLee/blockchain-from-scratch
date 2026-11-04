//! 第 6 章：mempool 和出塊。執行：`cargo run --example ch06-mempool`

use blockchain_from_scratch::chain::Blockchain;
use blockchain_from_scratch::ledger::Ledger;
use blockchain_from_scratch::mempool::Mempool;
use blockchain_from_scratch::miner::mine_next;
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::transaction::{Transaction, TxBlock};
use blockchain_from_scratch::wallet::Wallet;

fn push(chain: &mut Blockchain<Vec<Transaction>>, block: TxBlock) -> Ledger {
    chain.push(block).expect("mined on our own tip");
    Ledger::from_blocks(chain.blocks(), chain.params()).expect("every block was checked")
}

fn main() {
    let params = ChainParams::fixed_difficulty(12);
    let alice = Wallet::generate();
    let bob = Wallet::generate();
    let carol = Wallet::generate();
    let name = |address: &str| {
        if address == alice.address() {
            "Alice"
        } else if address == bob.address() {
            "Bob"
        } else if address == carol.address() {
            "Carol"
        } else {
            "?"
        }
    };

    let mut chain = Blockchain::new(params);
    println!("== Alice 挖到第 1 個區塊，拿到出塊獎勵 ==");
    let (block, attempts) = mine_next(
        &chain,
        &Mempool::default(),
        &alice.address(),
        1_700_000_000_000,
    );
    println!("試了 {attempts} 次雜湊");
    let ledger = push(&mut chain, block);
    println!("Alice 的餘額：{}", ledger.balance(&alice.address()));

    println!("\n== Alice 同時送出兩筆 30 元的轉帳 ==");
    let mut pool = Mempool::default();
    let to_bob = Transaction::transfer(&alice, &bob.address(), 30, 1);
    let to_carol = Transaction::transfer(&alice, &carol.address(), 30, 2);
    println!("給 Bob：  {:?}", pool.add(to_bob, &ledger));
    println!("給 Carol：{:?}", pool.add(to_carol, &ledger));
    println!("mempool 裡有 {} 筆交易", pool.len());

    println!("\n== Bob 挖下一個區塊，把 mempool 的交易打包進去 ==");
    let (block, _) = mine_next(&chain, &pool, &bob.address(), 1_700_000_010_000);
    println!(
        "區塊 #{} 有 {} 筆交易（第一筆是出塊獎勵）",
        block.header.index,
        block.data.len()
    );
    let ledger = push(&mut chain, block);
    let pool = Mempool::rebuild(pool.transactions().to_vec(), &ledger);
    println!("上鏈之後 mempool 剩 {} 筆", pool.len());
    for (address, balance) in ledger.balances() {
        println!("{:>6}：{balance}", name(address));
    }
}
