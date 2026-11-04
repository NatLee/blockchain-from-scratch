//! 第 8 章：分岔和共識
//!
//! 兩個礦工差不多同時挖到區塊時，網路會暫時分成兩邊，各自相信不同的最新區塊，這叫分岔（fork）。
//! 等其中一邊先挖到下一個區塊，大家就有了判斷的依據：
//!
//! > 相信累積工作量（total work）最多的那條鏈。
//!
//! 常聽到的「最長鏈原則」其實是這條規則的簡化版。區塊數一樣多，但難度比較高的鏈，
//! 背後花的計算比較多，比較難偽造，所以比的是工作量，不是區塊數。
//!
//! 換到另一條鏈叫「重組」（reorg）。被換掉的那幾個區塊裡的交易，如果新鏈上沒有，
//! 要放回 mempool 等下一次出塊，不然那些交易就憑空消失了。

use std::collections::HashSet;

use crate::block::Block;
use crate::transaction::{Transaction, TxBlock};

/// 兩條鏈從第幾個區塊開始不一樣（前面都一樣）。兩條鏈至少共用創世區塊，所以結果至少是 1。
pub fn fork_point<D>(a: &[Block<D>], b: &[Block<D>]) -> usize {
    a.iter()
        .zip(b)
        .take_while(|(x, y)| x.hash == y.hash)
        .count()
}

/// 要不要換到另一條鏈：只有對方的累積工作量比較多才換。一樣多就留在自己這條（先看到的優先）。
pub fn prefers(candidate_work: u128, current_work: u128) -> bool {
    candidate_work > current_work
}

/// 重組時，舊鏈被換掉的區塊裡、新鏈上沒有的交易（不含出塊獎勵）。這些要放回 mempool。
pub fn orphaned_transactions(old: &[TxBlock], new: &[TxBlock]) -> Vec<Transaction> {
    let fork = fork_point(old, new);
    let kept: HashSet<String> = new[fork..]
        .iter()
        .flat_map(|block| block.data.iter().map(Transaction::id))
        .collect();
    old[fork..]
        .iter()
        .flat_map(|block| block.data.iter())
        .filter(|tx| !matches!(tx, Transaction::Coinbase { .. }))
        .filter(|tx| !kept.contains(&tx.id()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Blockchain;
    use crate::params::ChainParams;
    use crate::pow::mine;
    use crate::wallet::Wallet;

    fn extend(
        chain: &mut Blockchain<Vec<Transaction>>,
        miner: &Wallet,
        txs: Vec<Transaction>,
        time: u64,
    ) {
        let height = chain.latest().header.index + 1;
        let mut data = vec![Transaction::coinbase(
            &miner.address(),
            chain.params().block_reward,
            height,
        )];
        data.extend(txs);
        let (block, _) = mine(chain.next_block(time, data));
        chain.push(block).unwrap();
    }

    #[test]
    fn finds_where_two_chains_split() {
        let params = ChainParams::fixed_difficulty(0);
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let mut base = Blockchain::new(params);
        extend(&mut base, &alice, vec![], 1_700_000_000_000);

        let mut a = base.clone();
        let mut b = base.clone();
        extend(&mut a, &alice, vec![], 1_700_000_001_000);
        extend(&mut b, &bob, vec![], 1_700_000_001_000);
        assert_eq!(fork_point(a.blocks(), b.blocks()), 2);
        assert_eq!(fork_point(a.blocks(), a.blocks()), 3);
    }

    #[test]
    fn orphaned_transactions_skip_coinbase_and_ones_already_in_the_new_chain() {
        let params = ChainParams::fixed_difficulty(0);
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let mut base = Blockchain::new(params);
        extend(&mut base, &alice, vec![], 1_700_000_000_000);

        let shared = Transaction::transfer(&alice, &bob.address(), 5, 1);
        let only_old = Transaction::transfer(&alice, &bob.address(), 7, 2);
        let mut old = base.clone();
        let mut new = base.clone();
        extend(
            &mut old,
            &alice,
            vec![shared.clone(), only_old.clone()],
            1_700_000_001_000,
        );
        extend(&mut new, &bob, vec![shared], 1_700_000_001_000);

        assert_eq!(
            orphaned_transactions(old.blocks(), new.blocks()),
            vec![only_old]
        );
    }

    #[test]
    fn ties_keep_the_current_chain() {
        assert!(!prefers(10, 10));
        assert!(prefers(11, 10));
    }
}
