//! 第 7 章（上）：節點之間的訊息
//!
//! 節點之間只傳四種訊息，跟原本的專案（和它參考的 naivechain）一樣：
//!
//! | 訊息 | 意思 | 原本的名稱 |
//! |---|---|---|
//! | `QUERY_LATEST` | 你最新的區塊是哪個？ | `QUERY_LATEST = 0` |
//! | `QUERY_ALL` | 把你整條鏈給我 | `QUERY_ALL = 1` |
//! | `RESPONSE_BLOCKCHAIN` | 這是我的區塊（一個或整條） | `RESPONSE_BLOCKCHAIN = 2` |
//! | `BROADCAST_TRANSACTION` | 有一筆新交易 | `BROADCASTED_MESSAGE = 3` |
//!
//! 訊息用 JSON 傳，例如 `{"type":"QUERY_LATEST"}`。

use serde::{Deserialize, Serialize};

use crate::transaction::{Transaction, TxBlock};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Message {
    QueryLatest,
    QueryAll,
    ResponseBlockchain { blocks: Vec<TxBlock> },
    BroadcastTransaction { transaction: Transaction },
}

impl Message {
    /// 給 log 和網頁看的簡短說明。
    pub fn summary(&self) -> String {
        match self {
            Message::QueryLatest => "QUERY_LATEST".to_string(),
            Message::QueryAll => "QUERY_ALL".to_string(),
            Message::ResponseBlockchain { blocks } => match blocks.as_slice() {
                [one] => format!("RESPONSE_BLOCKCHAIN #{}", one.header.index),
                many => format!("RESPONSE_BLOCKCHAIN ×{}", many.len()),
            },
            Message::BroadcastTransaction { transaction } => {
                format!("BROADCAST_TRANSACTION {}", &transaction.id()[..8])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_uses_the_original_style_type_names() {
        let json = serde_json::to_string(&Message::QueryLatest).unwrap();
        assert_eq!(json, r#"{"type":"QUERY_LATEST"}"#);
        let back: Message = serde_json::from_str(r#"{"type":"QUERY_ALL"}"#).unwrap();
        assert_eq!(back, Message::QueryAll);
    }
}
