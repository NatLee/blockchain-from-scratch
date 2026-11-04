//! 第 7 章（下）：節點
//!
//! 節點把前面幾章組起來：一條鏈、一本帳、一個 mempool、一個收出塊獎勵的錢包，
//! 再加上 FileChain 用的鏈金鑰（第 9 章）。
//!
//! 這個 `Node` 刻意不碰網路，也不看時鐘：
//!
//! - 收到訊息時呼叫 [`Node::handle`]，它回傳「要回覆誰、要廣播什麼」（[`Outbound`]），
//!   由呼叫的人負責真的送出去。
//! - 需要時間的地方（出塊）由呼叫的人傳進來。
//!
//! 這種寫法叫 sans-IO。好處是同一份程式可以放進真的節點程式（`src/node`，用 websocket 送訊息）、
//! 瀏覽器（`src/wasm`），也可以在測試裡用 [`crate::sim::Network`] 模擬整個網路。

use serde::{Deserialize, Serialize};

use crate::chain::{BlockError, Blockchain, ChainError};
use crate::consensus;
use crate::filechain::{AddData, ChainKey, FileError, FileHit};
use crate::ledger::{Ledger, LedgerError};
use crate::mempool::{Added, Mempool};
use crate::miner;
use crate::p2p::Message;
use crate::params::ChainParams;
use crate::pow;
use crate::transaction::{Transaction, TxBlock, TxError};
use crate::wallet::Wallet;

/// 原本 `POST /mineBlock` 在 pool 是空的時候回的訊息，沿用原文。
pub const EMPTY_POOL_MESSAGE: &str = "No data can be packed into block";

/// 節點處理完一件事之後要送出的訊息。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outbound {
    /// 只回給送訊息來的那個節點。
    Reply(Message),
    /// 送給所有連著的節點。
    Broadcast(Message),
}

/// 節點發生的事，給 log 和網頁看。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NodeEvent {
    TransactionAdded {
        id: String,
    },
    TransactionRejected {
        reason: String,
    },
    BlockAccepted {
        index: u64,
        hash: String,
    },
    BlockRejected {
        index: u64,
        reason: String,
    },
    /// 換到累積工作量比較多的另一條鏈。`orphaned` 是被換掉的區塊裡、新鏈沒有的交易數，
    /// `returned` 是其中重新檢查後還合法、放回 mempool 的交易數。
    ChainReplaced {
        old_height: u64,
        new_height: u64,
        fork_index: usize,
        orphaned: usize,
        returned: usize,
    },
    ChainIgnored {
        reason: String,
    },
}

impl std::fmt::Display for NodeEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeEvent::TransactionAdded { id } => write!(f, "收到新交易 {}", short(id)),
            NodeEvent::TransactionRejected { reason } => write!(f, "拒絕交易：{reason}"),
            NodeEvent::BlockAccepted { index, hash } => {
                write!(f, "接上區塊 #{index}（{}…）", short(hash))
            }
            NodeEvent::BlockRejected { index, reason } => write!(f, "拒絕區塊 #{index}：{reason}"),
            NodeEvent::ChainReplaced {
                old_height,
                new_height,
                fork_index,
                orphaned,
                returned,
            } => write!(
                f,
                "換到工作量比較多的鏈：高度 {old_height} → {new_height}，從區塊 #{fork_index} 開始不同；\
                 被換掉的區塊有 {orphaned} 筆交易，{returned} 筆重新放回 mempool"
            ),
            NodeEvent::ChainIgnored { reason } => write!(f, "不換鏈：{reason}"),
        }
    }
}

/// 雜湊太長，log 只印前 8 個字元。
fn short(hash: &str) -> &str {
    hash.get(..8).unwrap_or(hash)
}

/// 為什麼不接受一個區塊。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AcceptError {
    #[error(transparent)]
    Block(#[from] BlockError),
    #[error(transparent)]
    Ledger(#[from] LedgerError),
}

/// 為什麼不換鏈。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReplaceError {
    #[error(transparent)]
    Chain(#[from] ChainError),
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error("對方的累積工作量沒有比較多")]
    NotMoreWork,
}

/// `POST /mineBlock` 為什麼不出塊。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MineError {
    #[error("{}", EMPTY_POOL_MESSAGE)]
    EmptyPool,
    #[error("挖礦期間鏈已經更新了，這個區塊接不上：{0}")]
    Stale(AcceptError),
}

/// `GET /getPublicKey` 的回應。欄位名稱沿用原本的 `public_key`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeyResponse {
    pub public_key: String,
}

/// 原本 API 用的 `{"status": …, "message": …}` 格式。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// 一個區塊鏈節點（不含網路）。
pub struct Node {
    chain: Blockchain<Vec<Transaction>>,
    ledger: Ledger,
    mempool: Mempool,
    wallet: Wallet,
    chain_key: Option<ChainKey>,
    events: Vec<NodeEvent>,
}

impl Node {
    /// 新節點：只有創世區塊。`wallet` 收出塊獎勵；`chain_key` 是 FileChain 解密用的鏈私鑰，可以沒有。
    pub fn new(params: ChainParams, wallet: Wallet, chain_key: Option<ChainKey>) -> Self {
        Node {
            chain: Blockchain::new(params),
            ledger: Ledger::default(),
            mempool: Mempool::default(),
            wallet,
            chain_key,
            events: Vec::new(),
        }
    }

    pub fn chain(&self) -> &Blockchain<Vec<Transaction>> {
        &self.chain
    }

    pub fn blocks(&self) -> &[TxBlock] {
        self.chain.blocks()
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    pub fn mempool(&self) -> &Mempool {
        &self.mempool
    }

    /// 這個節點收出塊獎勵的地址。
    pub fn address(&self) -> String {
        self.wallet.address()
    }

    /// 礦工的錢包。模擬和範例用它簽轉帳；真的節點不會把它送到網路上。
    pub fn wallet(&self) -> &Wallet {
        &self.wallet
    }

    /// FileChain 的鏈公鑰。
    pub fn chain_public_key(&self) -> Option<String> {
        self.chain_key.as_ref().map(ChainKey::public_hex)
    }

    /// 拿走累積的事件（拿完就清空）。`std::mem::take` 把 Vec 換成空的，再把原本的交給呼叫者。
    pub fn drain_events(&mut self) -> Vec<NodeEvent> {
        std::mem::take(&mut self.events)
    }

    /// 剛跟別的節點連上線時先問：「你最新的區塊是哪個？」
    pub fn greeting() -> Message {
        Message::QueryLatest
    }

    /// 收到一筆新交易（從 API 或其他節點）：檢查後放進 mempool，並轉傳給其他節點。
    pub fn submit(&mut self, tx: Transaction) -> Result<Vec<Outbound>, TxError> {
        let id = tx.id();
        // `tx.clone()`：一份放進 mempool，一份留著做廣播訊息。
        match self.mempool.add(tx.clone(), &self.ledger) {
            Ok(Added::New) => {
                self.events.push(NodeEvent::TransactionAdded { id });
                Ok(vec![Outbound::Broadcast(Message::BroadcastTransaction {
                    transaction: tx,
                })])
            }
            Ok(Added::AlreadyKnown) => Ok(Vec::new()),
            Err(error) => {
                self.events.push(NodeEvent::TransactionRejected {
                    reason: error.to_string(),
                });
                Err(error)
            }
        }
    }

    /// `POST /addData`：FileChain 的檔案紀錄。
    pub fn add_data(&mut self, data: AddData) -> Result<Vec<Outbound>, TxError> {
        self.submit(Transaction::from(data))
    }

    /// 準備下一個區塊（還沒挖礦）。真的節點會在別的執行緒挖礦，挖礦期間不鎖住節點。
    pub fn block_template(&self, timestamp: u64) -> TxBlock {
        miner::assemble(&self.chain, &self.mempool, &self.address(), timestamp)
    }

    /// 接受自己挖到的區塊，並廣播給其他節點。
    pub fn accept_mined(&mut self, block: TxBlock) -> Result<Vec<Outbound>, AcceptError> {
        self.accept_block(block)?;
        Ok(vec![self.announce_latest()])
    }

    /// 準備區塊、挖礦、接受，一次做完（測試、模擬和瀏覽器用）。
    pub fn mine(&mut self, timestamp: u64) -> (TxBlock, Vec<Outbound>) {
        let (block, _) = pow::mine(self.block_template(timestamp));
        let outbound = self
            .accept_mined(block.clone())
            .expect("a block mined on top of our own tip is valid");
        (block, outbound)
    }

    /// `POST /mineBlock` 的規則（沿用原本的 API）：pool 是空的就不出塊。
    /// 回傳還沒挖礦的區塊，挖好之後交給 [`Node::accept_mined`]。
    pub fn pool_template(&self, timestamp: u64) -> Result<TxBlock, MineError> {
        if self.mempool.is_empty() {
            return Err(MineError::EmptyPool);
        }
        Ok(self.block_template(timestamp))
    }

    /// `POST /mineBlock` 一次做完：檢查 pool、準備、挖礦、接受。
    pub fn mine_pool(&mut self, timestamp: u64) -> Result<(TxBlock, Vec<Outbound>), MineError> {
        let (block, _) = pow::mine(self.pool_template(timestamp)?);
        let outbound = self.accept_mined(block.clone()).map_err(MineError::Stale)?;
        Ok((block, outbound))
    }

    /// `GET /queryBlockByName`：依名稱查檔案，並用鏈私鑰解密。
    pub fn query_by_name(&self, name: &str) -> Result<Vec<FileHit>, FileError> {
        let key = self.chain_key.as_ref().ok_or(FileError::NoChainKey)?;
        Ok(crate::filechain::query_by_name(self.blocks(), name, key))
    }

    /// 處理其他節點送來的訊息，回傳要送出的訊息。
    pub fn handle(&mut self, message: Message) -> Vec<Outbound> {
        match message {
            Message::QueryLatest => vec![Outbound::Reply(Message::ResponseBlockchain {
                blocks: vec![self.chain.latest().clone()],
            })],
            Message::QueryAll => vec![Outbound::Reply(Message::ResponseBlockchain {
                blocks: self.blocks().to_vec(),
            })],
            // 別人轉傳的交易不合法時，不回覆也不轉傳，只記一筆事件。
            Message::BroadcastTransaction { transaction } => {
                self.submit(transaction).unwrap_or_default()
            }
            Message::ResponseBlockchain { blocks } => self.handle_blocks(blocks),
        }
    }

    /// 收到別人的區塊。規則跟原本的專案一樣，只是「比較長」改成「累積工作量比較多」：
    ///
    /// 1. 只收到一個區塊，而且剛好接在我們最新的區塊後面：檢查後接上，再告訴其他節點。
    /// 2. 只收到一個區塊，但編號比我們新、又接不上：我們落後或分岔了，請對方給整條鏈。
    /// 3. 收到整條鏈：累積工作量比我們多，而且全部合法，就換過去（第 8 章）。
    /// 4. 其他情況（對方比較舊、或我們已經有了）：不理它。
    fn handle_blocks(&mut self, mut blocks: Vec<TxBlock>) -> Vec<Outbound> {
        blocks.sort_by_key(|b| b.header.index);

        // 先決定要做什麼（只讀），再動手（會修改）。分成兩步，借用檢查器（borrow checker）
        // 才不會抱怨「一邊讀 self、一邊改 self」。
        enum Plan {
            Ignore,
            Append,
            AskForAll,
            Replace,
        }
        let plan = {
            let our_latest = self.chain.latest();
            match blocks.as_slice() {
                [] => Plan::Ignore,
                [block] if self.blocks().iter().any(|b| b.hash == block.hash) => Plan::Ignore,
                [block] if block.header.previous_hash == our_latest.hash => Plan::Append,
                [block] if block.header.index > our_latest.header.index => Plan::AskForAll,
                [_] => Plan::Ignore,
                _ => Plan::Replace,
            }
        };

        match plan {
            Plan::Ignore => Vec::new(),
            Plan::AskForAll => vec![Outbound::Reply(Message::QueryAll)],
            Plan::Append => {
                // `pop` 把區塊從 Vec 拿出來（所有權轉給我們），不用再複製一份。
                let block = blocks.pop().expect("exactly one block");
                match self.accept_block(block) {
                    Ok(()) => vec![self.announce_latest()],
                    Err(_) => Vec::new(),
                }
            }
            Plan::Replace => match self.replace_chain(blocks) {
                Ok(()) => vec![self.announce_latest()],
                Err(_) => Vec::new(),
            },
        }
    }

    fn announce_latest(&self) -> Outbound {
        Outbound::Broadcast(Message::ResponseBlockchain {
            blocks: vec![self.chain.latest().clone()],
        })
    }

    /// 檢查一個要接在最新區塊後面的區塊：先在帳本的副本上試算交易，全部通過才真的接上。
    fn accept_block(&mut self, block: TxBlock) -> Result<(), AcceptError> {
        let index = block.header.index;
        let hash = block.hash.clone();
        let result = self.try_accept_block(block);
        match &result {
            Ok(()) => {
                // 已經上鏈的交易從 mempool 拿掉，剩下的重新檢查一次。
                let pending = std::mem::take(&mut self.mempool);
                self.mempool = Mempool::rebuild(pending.transactions().to_vec(), &self.ledger);
                self.events.push(NodeEvent::BlockAccepted { index, hash });
            }
            Err(error) => self.events.push(NodeEvent::BlockRejected {
                index,
                reason: error.to_string(),
            }),
        }
        result
    }

    fn try_accept_block(&mut self, block: TxBlock) -> Result<(), AcceptError> {
        // 在副本上試算：有一筆交易不合法，原本的帳本就完全不受影響。
        let mut ledger = self.ledger.clone();
        ledger.apply_block(&block, self.chain.params())?;
        self.chain.push(block)?;
        self.ledger = ledger;
        Ok(())
    }

    /// 換成另一條累積工作量比較多的鏈（重組）。
    fn replace_chain(&mut self, blocks: Vec<TxBlock>) -> Result<(), ReplaceError> {
        let result = self.try_replace_chain(blocks);
        if let Err(error) = &result {
            self.events.push(NodeEvent::ChainIgnored {
                reason: error.to_string(),
            });
        }
        result
    }

    fn try_replace_chain(&mut self, blocks: Vec<TxBlock>) -> Result<(), ReplaceError> {
        let candidate = Blockchain::from_blocks(blocks, self.chain.params().clone())?;
        if !consensus::prefers(candidate.total_work(), self.chain.total_work()) {
            return Err(ReplaceError::NotMoreWork);
        }
        let ledger = Ledger::from_blocks(candidate.blocks(), candidate.params())?;

        let fork_index = consensus::fork_point(self.blocks(), candidate.blocks());
        let orphaned = consensus::orphaned_transactions(self.blocks(), candidate.blocks());
        let orphaned_ids: Vec<String> = orphaned.iter().map(Transaction::id).collect();
        // 被換掉的交易排在前面（它們比較早送出），再接原本 mempool 裡的交易。
        let pending = std::mem::take(&mut self.mempool);
        let candidates = orphaned
            .into_iter()
            .chain(pending.transactions().iter().cloned());
        self.mempool = Mempool::rebuild(candidates, &ledger);
        let returned = self
            .mempool
            .transactions()
            .iter()
            .filter(|tx| orphaned_ids.contains(&tx.id()))
            .count();

        let old_height = self.chain.latest().header.index;
        self.chain = candidate;
        self.ledger = ledger;
        self.events.push(NodeEvent::ChainReplaced {
            old_height,
            new_height: self.chain.latest().header.index,
            fork_index,
            orphaned: orphaned_ids.len(),
            returned,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> Node {
        Node::new(
            ChainParams::fixed_difficulty(4),
            Wallet::generate(),
            Some(ChainKey::generate()),
        )
    }

    #[test]
    fn answers_queries() {
        let mut a = node();
        let replies = a.handle(Message::QueryLatest);
        assert!(
            matches!(&replies[..], [Outbound::Reply(Message::ResponseBlockchain { blocks })] if blocks.len() == 1)
        );
        a.mine(1_700_000_000_000);
        let replies = a.handle(Message::QueryAll);
        assert!(
            matches!(&replies[..], [Outbound::Reply(Message::ResponseBlockchain { blocks })] if blocks.len() == 2)
        );
    }

    #[test]
    fn appends_a_block_that_extends_our_tip() {
        let mut a = node();
        let mut b = node();
        let (block, _) = a.mine(1_700_000_000_000);
        let out = b.handle(Message::ResponseBlockchain {
            blocks: vec![block.clone()],
        });
        assert_eq!(b.chain().latest().hash, block.hash);
        assert!(matches!(&out[..], [Outbound::Broadcast(_)]));
        // 再收到一次同一個區塊：已經有了，不理它。
        assert!(
            b.handle(Message::ResponseBlockchain {
                blocks: vec![block]
            })
            .is_empty()
        );
    }

    #[test]
    fn asks_for_the_whole_chain_when_behind() {
        let mut a = node();
        let mut b = node();
        a.mine(1_700_000_000_000);
        let (latest, _) = a.mine(1_700_000_001_000);
        let out = b.handle(Message::ResponseBlockchain {
            blocks: vec![latest],
        });
        assert_eq!(out, vec![Outbound::Reply(Message::QueryAll)]);
        b.handle(Message::ResponseBlockchain {
            blocks: a.blocks().to_vec(),
        });
        assert_eq!(b.chain().len(), 3);
    }

    #[test]
    fn empty_pool_keeps_the_original_api_message() {
        let mut a = node();
        assert_eq!(a.mine_pool(1).unwrap_err().to_string(), EMPTY_POOL_MESSAGE);
    }

    #[test]
    fn rejects_a_block_with_a_bad_transaction() {
        let a = node();
        let mut b = node();
        // Alice 沒有錢卻轉帳；直接塞進區塊（繞過 mempool 的檢查）。
        let alice = Wallet::generate();
        let mut template = a.block_template(1_700_000_000_000);
        template
            .data
            .push(Transaction::transfer(&alice, &alice.address(), 10, 1));
        template.rehash();
        let (block, _) = pow::mine(template);
        b.handle(Message::ResponseBlockchain {
            blocks: vec![block],
        });
        assert_eq!(b.chain().len(), 1);
        assert!(matches!(
            b.drain_events().last(),
            Some(NodeEvent::BlockRejected { .. })
        ));
    }
}
