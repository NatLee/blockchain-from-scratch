//! 第 6 章（上）：帳本（ledger）
//!
//! 區塊鏈本身只記「發生過哪些交易」，沒有存「每個人有多少錢」。
//! 餘額是從創世區塊開始，把每筆交易照順序套用一遍算出來的。
//! 套用的時候順便檢查：簽章對不對、錢夠不夠、有沒有重複。

use std::collections::{BTreeMap, HashSet};

use crate::params::ChainParams;
use crate::transaction::{Transaction, TxBlock, TxError};
use crate::wallet::is_address;

/// 區塊裡的交易不合法。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LedgerError {
    #[error("區塊 {index} 的第一筆必須是出塊獎勵：{reward} 給礦工，height 等於區塊編號")]
    BadCoinbase { index: u64, reward: u64 },
    #[error("區塊 {index} 的第 {position} 筆交易不合法：{error}")]
    BadTransaction {
        index: u64,
        position: usize,
        error: TxError,
    },
}

/// 目前的帳本狀態：每個地址的餘額，以及已經上鏈的交易編號。
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    balances: BTreeMap<String, u64>,
    seen: HashSet<String>,
}

impl Ledger {
    /// 從創世區塊開始重算整條鏈的帳本。
    pub fn from_blocks(blocks: &[TxBlock], params: &ChainParams) -> Result<Self, LedgerError> {
        let mut ledger = Ledger::default();
        // 創世區塊沒有交易，從第 1 個區塊開始。
        for block in blocks.iter().skip(1) {
            ledger.apply_block(block, params)?;
        }
        Ok(ledger)
    }

    pub fn balance(&self, address: &str) -> u64 {
        self.balances.get(address).copied().unwrap_or(0)
    }

    /// 所有有錢的地址和餘額，依地址排序。
    pub fn balances(&self) -> &BTreeMap<String, u64> {
        &self.balances
    }

    /// 這筆交易是不是已經上鏈了。
    pub fn contains(&self, tx_id: &str) -> bool {
        self.seen.contains(tx_id)
    }

    /// 檢查一筆一般交易（不是出塊獎勵）能不能套用在目前的狀態上。
    pub fn check(&self, tx: &Transaction) -> Result<(), TxError> {
        if self.contains(&tx.id()) {
            return Err(TxError::Duplicate);
        }
        match tx {
            Transaction::Coinbase { .. } => Err(TxError::UnexpectedCoinbase),
            Transaction::Transfer(signed) => {
                signed.verify()?;
                let transfer = &signed.body;
                if transfer.amount == 0 {
                    return Err(TxError::ZeroAmount);
                }
                if !is_address(&transfer.to) {
                    return Err(TxError::BadAddress);
                }
                let balance = self.balance(&signed.public_key);
                if balance < transfer.amount {
                    return Err(TxError::InsufficientFunds {
                        balance,
                        amount: transfer.amount,
                    });
                }
                Ok(())
            }
            Transaction::File(signed) => {
                signed.verify()?;
                signed.body.validate()
            }
        }
    }

    /// 檢查後套用一筆一般交易。
    pub fn apply(&mut self, tx: &Transaction) -> Result<(), TxError> {
        self.check(tx)?;
        self.apply_unchecked(tx);
        Ok(())
    }

    /// 套用一整個區塊。第一筆必須是出塊獎勵，其餘照順序檢查和套用。
    ///
    /// 中途失敗時，帳本可能已經套用了一半；呼叫的人應該在副本上呼叫，成功才換掉原本的帳本
    /// （`Node` 就是這樣做的）。
    pub fn apply_block(
        &mut self,
        block: &TxBlock,
        params: &ChainParams,
    ) -> Result<(), LedgerError> {
        let index = block.header.index;
        let bad_coinbase = LedgerError::BadCoinbase {
            index,
            reward: params.block_reward,
        };
        let Some((first, rest)) = block.data.split_first() else {
            return Err(bad_coinbase);
        };
        match first {
            Transaction::Coinbase { to, amount, height }
                if *amount == params.block_reward && *height == index && is_address(to) => {}
            _ => return Err(bad_coinbase),
        }
        self.apply_unchecked(first);
        for (i, tx) in rest.iter().enumerate() {
            self.apply(tx)
                .map_err(|error| LedgerError::BadTransaction {
                    index,
                    position: i + 1,
                    error,
                })?;
        }
        Ok(())
    }

    fn apply_unchecked(&mut self, tx: &Transaction) {
        match tx {
            Transaction::Coinbase { to, amount, .. } => self.credit(to, *amount),
            Transaction::Transfer(signed) => {
                let transfer = &signed.body;
                // `check` 已經確認餘額夠，這裡用 `-=` 不會變成負數。
                *self.balances.entry(signed.public_key.clone()).or_default() -= transfer.amount;
                self.credit(&transfer.to, transfer.amount);
            }
            // 檔案存證不動到餘額。
            Transaction::File(_) => {}
        }
        self.seen.insert(tx.id());
    }

    fn credit(&mut self, address: &str, amount: u64) {
        let balance = self.balances.entry(address.to_string()).or_default();
        *balance = balance.saturating_add(amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Blockchain;
    use crate::pow::mine;
    use crate::wallet::Wallet;

    fn params() -> ChainParams {
        ChainParams::fixed_difficulty(0)
    }

    fn mine_block(chain: &mut Blockchain<Vec<Transaction>>, miner: &Wallet, txs: Vec<Transaction>) {
        let height = chain.latest().header.index + 1;
        let mut data = vec![Transaction::coinbase(
            &miner.address(),
            chain.params().block_reward,
            height,
        )];
        data.extend(txs);
        let (block, _) = mine(chain.next_block(1_700_000_000_000 + height, data));
        chain.push(block).unwrap();
    }

    #[test]
    fn balances_come_from_replaying_the_chain() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let mut chain = Blockchain::new(params());
        mine_block(&mut chain, &alice, vec![]);
        mine_block(
            &mut chain,
            &bob,
            vec![Transaction::transfer(&alice, &bob.address(), 30, 1)],
        );

        let ledger = Ledger::from_blocks(chain.blocks(), chain.params()).unwrap();
        assert_eq!(ledger.balance(&alice.address()), 20);
        assert_eq!(ledger.balance(&bob.address()), 80);
    }

    #[test]
    fn cannot_spend_more_than_you_have() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let ledger = Ledger::default();
        let tx = Transaction::transfer(&alice, &bob.address(), 1, 1);
        assert_eq!(
            ledger.check(&tx),
            Err(TxError::InsufficientFunds {
                balance: 0,
                amount: 1
            })
        );
    }

    #[test]
    fn rejects_the_same_transaction_twice() {
        let alice = Wallet::generate();
        let mut ledger = Ledger::default();
        ledger.credit(&alice.address(), 100);
        let tx = Transaction::transfer(&alice, &Wallet::generate().address(), 10, 1);
        ledger.apply(&tx).unwrap();
        assert_eq!(ledger.apply(&tx), Err(TxError::Duplicate));
    }

    #[test]
    fn a_block_needs_exactly_the_right_reward_first() {
        let miner = Wallet::generate();
        let p = params();
        let mut chain: Blockchain<Vec<Transaction>> = Blockchain::new(p.clone());
        let greedy = vec![Transaction::coinbase(
            &miner.address(),
            p.block_reward * 10,
            1,
        )];
        let (block, _) = mine(chain.next_block(1_700_000_000_000, greedy));
        chain.push(block).unwrap();
        assert_eq!(
            Ledger::from_blocks(chain.blocks(), &p).unwrap_err(),
            LedgerError::BadCoinbase {
                index: 1,
                reward: p.block_reward
            }
        );
    }

    #[test]
    fn coinbase_is_not_a_normal_transaction() {
        let tx = Transaction::coinbase(&Wallet::generate().address(), 50, 1);
        assert_eq!(
            Ledger::default().check(&tx),
            Err(TxError::UnexpectedCoinbase)
        );
    }

    #[test]
    fn rejects_zero_amount_and_bad_address() {
        let alice = Wallet::generate();
        let mut ledger = Ledger::default();
        ledger.credit(&alice.address(), 100);
        let zero = Transaction::transfer(&alice, &alice.address(), 0, 1);
        assert_eq!(ledger.check(&zero), Err(TxError::ZeroAmount));
        let bad = Transaction::transfer(&alice, "not-an-address", 5, 1);
        assert_eq!(ledger.check(&bad), Err(TxError::BadAddress));
    }
}
