//! 第 6 章（中）：mempool（記憶體池）
//!
//! 交易送到節點之後不會馬上上鏈，而是先放進 mempool 排隊，等礦工把它們打包進區塊。
//! 放進來之前要先檢查：簽章、重複、餘額。
//!
//! 檢查餘額時要把 mempool 裡還在排隊的交易也算進去。不然 Alice 只有 50 元，
//! 卻可以連送兩筆各 40 元的轉帳，兩筆都通過檢查。這就是「雙重支付」（double spending）。

use std::collections::HashSet;

use crate::ledger::Ledger;
use crate::transaction::{Transaction, TxError};

/// 加進 mempool 的結果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Added {
    /// 新的交易，已經放進 mempool。節點應該轉傳給其他節點。
    New,
    /// 已經在 mempool 或已經上鏈。不用再轉傳，避免訊息在網路裡無限繞圈。
    AlreadyKnown,
}

/// 等待上鏈的交易，依收到的順序排列。
#[derive(Clone, Debug, Default)]
pub struct Mempool {
    txs: Vec<Transaction>,
    ids: HashSet<String>,
}

impl Mempool {
    /// 檢查後加入一筆交易。`ledger` 是目前鏈上的帳本。
    ///
    /// `tx: Transaction` 會被搬進 mempool（所有權轉移）；檢查失敗時交易就被丟掉了。
    pub fn add(&mut self, tx: Transaction, ledger: &Ledger) -> Result<Added, TxError> {
        let id = tx.id();
        if self.ids.contains(&id) || ledger.contains(&id) {
            return Ok(Added::AlreadyKnown);
        }
        self.pending_ledger(ledger).check(&tx)?;
        self.ids.insert(id);
        self.txs.push(tx);
        Ok(Added::New)
    }

    /// 排隊中的交易，依收到的順序。
    pub fn transactions(&self) -> &[Transaction] {
        &self.txs
    }

    pub fn len(&self) -> usize {
        self.txs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.txs.is_empty()
    }

    /// 用一批候選交易重建 mempool：照順序重新檢查，只留下在 `ledger` 上還合法的。
    ///
    /// 新區塊上鏈後（已經上鏈的交易會被當成重複而拿掉），或換了一條鏈之後（第 8 章）都會呼叫。
    pub fn rebuild(candidates: impl IntoIterator<Item = Transaction>, ledger: &Ledger) -> Self {
        let mut pool = Mempool::default();
        for tx in candidates {
            // 檢查失敗的交易直接丟掉：例如另一條鏈上已經花掉了同一筆錢。
            let _ = pool.add(tx, ledger);
        }
        pool
    }

    /// 鏈上的帳本，再加上 mempool 裡所有排隊中的交易。
    fn pending_ledger(&self, ledger: &Ledger) -> Ledger {
        // `clone` 複製一份帳本來試算，不會動到原本的帳本。
        let mut pending = ledger.clone();
        for tx in &self.txs {
            // 每筆交易在加進來時都檢查過，這裡一定成功。
            let _ = pending.apply(tx);
        }
        pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Blockchain;
    use crate::params::ChainParams;
    use crate::pow::mine;
    use crate::wallet::Wallet;

    /// 給 `owner` 50 元的帳本（挖一個區塊拿出塊獎勵）。
    fn ledger_with_reward(owner: &Wallet) -> Ledger {
        let params = ChainParams::fixed_difficulty(0);
        let mut chain: Blockchain<Vec<Transaction>> = Blockchain::new(params.clone());
        let data = vec![Transaction::coinbase(
            &owner.address(),
            params.block_reward,
            1,
        )];
        let (block, _) = mine(chain.next_block(1_700_000_000_000, data));
        chain.push(block).unwrap();
        Ledger::from_blocks(chain.blocks(), &params).unwrap()
    }

    #[test]
    fn pending_transactions_count_against_the_balance() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let ledger = ledger_with_reward(&alice);
        let mut pool = Mempool::default();

        let first = Transaction::transfer(&alice, &bob.address(), 40, 1);
        let second = Transaction::transfer(&alice, &bob.address(), 40, 2);
        assert_eq!(pool.add(first, &ledger), Ok(Added::New));
        assert_eq!(
            pool.add(second, &ledger),
            Err(TxError::InsufficientFunds {
                balance: 10,
                amount: 40
            })
        );
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn duplicates_are_known_not_errors() {
        let alice = Wallet::generate();
        let ledger = ledger_with_reward(&alice);
        let mut pool = Mempool::default();
        let tx = Transaction::transfer(&alice, &Wallet::generate().address(), 5, 1);
        assert_eq!(pool.add(tx.clone(), &ledger), Ok(Added::New));
        assert_eq!(pool.add(tx, &ledger), Ok(Added::AlreadyKnown));
    }

    #[test]
    fn rebuild_drops_transactions_that_no_longer_fit() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let rich = ledger_with_reward(&alice);
        let poor = Ledger::default();
        let tx = Transaction::transfer(&alice, &bob.address(), 10, 1);
        assert_eq!(Mempool::rebuild([tx.clone()], &rich).len(), 1);
        assert!(Mempool::rebuild([tx], &poor).is_empty());
    }
}
