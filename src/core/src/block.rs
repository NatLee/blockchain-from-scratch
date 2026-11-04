//! 第 2 章：區塊（block）
//!
//! 一個區塊分成兩部分：
//!
//! - **標頭（header）**：編號、前一個區塊的雜湊、時間、內容的雜湊、難度、nonce。
//! - **內容（data）**：真正要記錄的東西。前幾章用一段文字，第 5 章開始是一串交易。
//!
//! 區塊的雜湊只算標頭。標頭裡有「內容的雜湊」（`data_hash`），所以內容改一個字，
//! `data_hash` 就變，區塊的雜湊也跟著變。這樣做的好處是：不管內容多大，算區塊雜湊時
//! 只要處理一小段固定長度的標頭。第 4 章挖礦要算幾百萬次雜湊，這點很重要。

use serde::{Deserialize, Serialize};

use crate::hash::sha256_hex;

/// 創世區塊的時間：2018-06-22 02:13:17（UTC+8），也就是原本專案第一個 commit 的時間。
/// 單位是毫秒（從 1970-01-01 UTC 起算）。
pub const GENESIS_TIMESTAMP: u64 = 1_529_604_797_000;

/// 創世區塊沒有前一個區塊，`previous_hash` 用 64 個 0 代替。
pub const GENESIS_PREVIOUS_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// 區塊標頭：挖礦時只會動到這裡的 `nonce`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    /// 第幾個區塊，創世區塊是 0。
    pub index: u64,
    /// 前一個區塊的雜湊。區塊就是靠這個欄位串成「鏈」。
    pub previous_hash: String,
    /// 出塊時間，毫秒。
    pub timestamp: u64,
    /// 內容（`data`）的 SHA-256。
    pub data_hash: String,
    /// 難度：區塊雜湊開頭至少要有幾個 0 bit（第 4 章）。
    pub difficulty: u32,
    /// 挖礦時一直換的數字（number used once），直到雜湊符合難度。
    pub nonce: u64,
}

impl Header {
    /// 要拿去雜湊的字串：標頭的每個欄位用 `|` 接起來。
    ///
    /// 印出來長這樣：`1|9f86…|1700000000000|2c26…|16|42`。
    /// 欄位之間要有分隔符號，不然 `1` + `23` 和 `12` + `3` 會變成同一個字串。
    pub fn hash_input(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.index,
            self.previous_hash,
            self.timestamp,
            self.data_hash,
            self.difficulty,
            self.nonce
        )
    }

    /// 這個標頭的 SHA-256，也就是區塊的雜湊。
    pub fn compute_hash(&self) -> String {
        sha256_hex(self.hash_input().as_bytes())
    }
}

/// 一個區塊。`D` 是內容的型別：前幾章是 `String`，第 5 章以後是 `Vec<Transaction>`。
///
/// 用泛型（generic）是為了讓同一套區塊和鏈的程式，可以裝不同的內容。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block<D> {
    /// `flatten`：轉成 JSON 時，標頭的欄位直接攤平在區塊裡，讀起來比較直覺。
    #[serde(flatten)]
    pub header: Header,
    /// 存下來的區塊雜湊。驗證時會重算一次，看跟這裡一不一樣。
    pub hash: String,
    pub data: D,
}

/// 內容的雜湊：把內容轉成 JSON，再算 SHA-256。
///
/// Rust 的 struct 轉 JSON 時欄位順序固定，所以同樣的內容一定得到同樣的雜湊。
pub fn data_hash<D: Serialize>(data: &D) -> String {
    let json = serde_json::to_vec(data).expect("block data always serializes to JSON");
    sha256_hex(&json)
}

impl<D: Serialize> Block<D> {
    /// 做出一個還沒挖礦（`nonce = 0`）的區塊。
    ///
    /// `data: D` 沒有 `&`，代表這個函式會「拿走」內容的所有權（ownership），
    /// 之後內容就屬於這個區塊，呼叫者不能再用原本的變數。
    pub fn new(
        index: u64,
        previous_hash: String,
        timestamp: u64,
        difficulty: u32,
        data: D,
    ) -> Self {
        let header = Header {
            index,
            previous_hash,
            timestamp,
            data_hash: data_hash(&data),
            difficulty,
            nonce: 0,
        };
        let hash = header.compute_hash();
        Block { header, hash, data }
    }

    /// 創世區塊：每個節點都用同一個，所以全部欄位都是固定的。
    pub fn genesis(data: D) -> Self {
        Self::new(
            0,
            GENESIS_PREVIOUS_HASH.to_string(),
            GENESIS_TIMESTAMP,
            0,
            data,
        )
    }

    /// 內容或標頭改過之後，重新算 `data_hash` 和 `hash`。
    ///
    /// `&mut self` 是「可變借用」：函式可以改這個區塊，但不會拿走它。
    pub fn rehash(&mut self) {
        self.header.data_hash = data_hash(&self.data);
        self.hash = self.header.compute_hash();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_covers_every_header_field() {
        let block = Block::new(
            1,
            "ab".repeat(32),
            1_700_000_000_000,
            0,
            "hello".to_string(),
        );
        let original = block.hash.clone();

        let mut changed = block.clone();
        changed.header.nonce += 1;
        assert_ne!(changed.header.compute_hash(), original);

        let mut changed = block.clone();
        changed.header.timestamp += 1;
        assert_ne!(changed.header.compute_hash(), original);
    }

    #[test]
    fn changing_data_changes_data_hash_and_block_hash() {
        let mut block = Block::new(
            1,
            "ab".repeat(32),
            1_700_000_000_000,
            0,
            "hello".to_string(),
        );
        let before = block.clone();
        block.data = "hellp".to_string();
        block.rehash();
        assert_ne!(block.header.data_hash, before.header.data_hash);
        assert_ne!(block.hash, before.hash);
    }

    #[test]
    fn genesis_is_deterministic() {
        assert_eq!(Block::genesis(String::new()), Block::genesis(String::new()));
        let genesis = Block::genesis(String::new());
        assert_eq!(genesis.header.index, 0);
        assert_eq!(genesis.header.previous_hash, GENESIS_PREVIOUS_HASH);
    }

    #[test]
    fn json_has_flat_header_fields() {
        let block = Block::genesis(String::new());
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["index"], 0);
        assert_eq!(json["previousHash"], GENESIS_PREVIOUS_HASH);
        let back: Block<String> = serde_json::from_value(json).unwrap();
        assert_eq!(back, block);
    }
}
