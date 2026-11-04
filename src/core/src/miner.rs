//! 第 6 章（下）：出塊
//!
//! 礦工從 mempool 拿交易，前面加一筆給自己的出塊獎勵，組成新區塊，再做工作量證明。
//! 出塊獎勵就是礦工願意花電費挖礦的原因。

use crate::chain::Blockchain;
use crate::mempool::Mempool;
use crate::pow;
use crate::transaction::{Transaction, TxBlock};

/// 準備下一個區塊（還沒挖礦）：第一筆是給 `miner` 的出塊獎勵，後面接 mempool 裡的交易。
pub fn assemble(
    chain: &Blockchain<Vec<Transaction>>,
    mempool: &Mempool,
    miner: &str,
    timestamp: u64,
) -> TxBlock {
    let height = chain.latest().header.index + 1;
    let mut data = Vec::with_capacity(mempool.len() + 1);
    data.push(Transaction::coinbase(
        miner,
        chain.params().block_reward,
        height,
    ));
    // `iter().cloned()`：複製一份交易放進區塊，mempool 裡的交易留著，等區塊真的上鏈才拿掉。
    data.extend(mempool.transactions().iter().cloned());
    chain.next_block(timestamp, data)
}

/// 準備下一個區塊並挖礦。回傳挖好的區塊和試了幾次雜湊。
pub fn mine_next(
    chain: &Blockchain<Vec<Transaction>>,
    mempool: &Mempool,
    miner: &str,
    timestamp: u64,
) -> (TxBlock, u64) {
    pow::mine(assemble(chain, mempool, miner, timestamp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Ledger;
    use crate::params::ChainParams;
    use crate::wallet::Wallet;

    #[test]
    fn mined_block_pays_the_miner_and_includes_the_pool() {
        let params = ChainParams::fixed_difficulty(4);
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let mut chain = Blockchain::new(params.clone());

        // Alice 先挖一個區塊拿 50 元。
        let (block, _) = mine_next(
            &chain,
            &Mempool::default(),
            &alice.address(),
            1_700_000_000_000,
        );
        chain.push(block).unwrap();
        let ledger = Ledger::from_blocks(chain.blocks(), &params).unwrap();

        // 轉 20 元給 Bob，由 Bob 挖下一個區塊。
        let mut pool = Mempool::default();
        pool.add(
            Transaction::transfer(&alice, &bob.address(), 20, 1),
            &ledger,
        )
        .unwrap();
        let (block, attempts) = mine_next(&chain, &pool, &bob.address(), 1_700_000_001_000);
        assert!(attempts >= 1);
        assert_eq!(block.data.len(), 2);
        chain.push(block).unwrap();

        let ledger = Ledger::from_blocks(chain.blocks(), &params).unwrap();
        assert_eq!(ledger.balance(&alice.address()), 30);
        assert_eq!(ledger.balance(&bob.address()), 70);
    }
}
