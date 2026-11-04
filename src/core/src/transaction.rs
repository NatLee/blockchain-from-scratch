//! 第 5 章（下）：交易
//!
//! 交易是「要寫進區塊鏈的一筆請求」，由發起的人簽章。這裡有三種：
//!
//! - **出塊獎勵（coinbase）**：礦工挖到區塊時給自己的錢，不用簽章，但只能放在區塊第一筆（第 6 章）。
//! - **轉帳（transfer）**：從簽章者的地址轉錢到另一個地址。
//! - **檔案存證（file）**：FileChain 的檔案紀錄（第 9 章）。
//!
//! 簽章涵蓋整個 `body`：金額、收款人、時間任何一個被改，簽章就對不上。

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::filechain::FileRecord;
use crate::hash::sha256_hex;
use crate::wallet::{self, Wallet};

/// 放交易的區塊。
pub type TxBlock = Block<Vec<Transaction>>;

/// 交易為什麼不能接受。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TxError {
    #[error("簽章不對：內容被改過，或不是這把公鑰簽的")]
    BadSignature,
    #[error("餘額不足：有 {balance}，要轉 {amount}")]
    InsufficientFunds { balance: u64, amount: u64 },
    #[error("金額必須大於 0")]
    ZeroAmount,
    #[error("收款地址的格式不對")]
    BadAddress,
    #[error("這筆交易已經處理過了")]
    Duplicate,
    #[error("出塊獎勵只能由礦工放在區塊的第一筆")]
    UnexpectedCoinbase,
    #[error("檔案紀錄的格式不對：{0}")]
    BadFile(String),
}

/// 簽過章的內容：內容本身、簽章者的公鑰、簽章。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signed<T> {
    pub body: T,
    pub public_key: String,
    pub signature: String,
}

/// 要簽章的 bytes：內容的 JSON。struct 轉 JSON 時欄位順序固定，所以簽的人和驗的人算出同樣的 bytes。
pub fn signing_bytes<T: Serialize>(body: &T) -> Vec<u8> {
    serde_json::to_vec(body).expect("transaction bodies always serialize to JSON")
}

impl<T: Serialize> Signed<T> {
    /// 用錢包簽 `body`。
    pub fn sign(body: T, wallet: &Wallet) -> Self {
        let signature = wallet.sign(&signing_bytes(&body));
        Signed {
            body,
            public_key: wallet.address(),
            signature,
        }
    }

    /// 驗證簽章。
    pub fn verify(&self) -> Result<(), TxError> {
        wallet::verify(
            &self.public_key,
            &signing_bytes(&self.body),
            &self.signature,
        )
        .map_err(|_| TxError::BadSignature)
    }
}

/// 轉帳的內容。付款人就是簽章者（`Signed::public_key`）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub to: String,
    pub amount: u64,
    /// 毫秒。同一個人轉同樣金額給同一個人兩次，靠時間讓兩筆交易的編號不同。
    pub timestamp: u64,
}

/// 一筆交易。
///
/// `#[serde(tag = "type")]`：轉成 JSON 時加一個 `"type"` 欄位標明是哪一種，例如 `{"type":"transfer",…}`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Transaction {
    Coinbase {
        to: String,
        amount: u64,
        height: u64,
    },
    Transfer(Signed<Transfer>),
    File(Signed<FileRecord>),
}

impl Transaction {
    /// 簽一筆轉帳。
    pub fn transfer(from: &Wallet, to: &str, amount: u64, timestamp: u64) -> Self {
        Transaction::Transfer(Signed::sign(
            Transfer {
                to: to.to_string(),
                amount,
                timestamp,
            },
            from,
        ))
    }

    /// 出塊獎勵。`height` 是區塊編號，讓每個區塊的獎勵交易編號都不一樣。
    pub fn coinbase(to: &str, amount: u64, height: u64) -> Self {
        Transaction::Coinbase {
            to: to.to_string(),
            amount,
            height,
        }
    }

    /// 交易編號：整筆交易（含簽章）的 SHA-256。
    pub fn id(&self) -> String {
        sha256_hex(&signing_bytes(self))
    }

    /// 發起人的地址；出塊獎勵沒有發起人。
    pub fn sender(&self) -> Option<&str> {
        match self {
            Transaction::Coinbase { .. } => None,
            Transaction::Transfer(signed) => Some(&signed.public_key),
            Transaction::File(signed) => Some(&signed.public_key),
        }
    }

    /// 檢查簽章。出塊獎勵沒有簽章，由區塊的規則檢查（見 `Ledger::apply_block`）。
    pub fn verify_signature(&self) -> Result<(), TxError> {
        match self {
            Transaction::Coinbase { .. } => Ok(()),
            Transaction::Transfer(signed) => signed.verify(),
            Transaction::File(signed) => signed.verify(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_transfer_verifies() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let tx = Transaction::transfer(&alice, &bob.address(), 10, 1);
        assert_eq!(tx.verify_signature(), Ok(()));
        assert_eq!(tx.sender(), Some(alice.address().as_str()));
    }

    #[test]
    fn changing_the_amount_breaks_the_signature() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let Transaction::Transfer(mut signed) =
            Transaction::transfer(&alice, &bob.address(), 10, 1)
        else {
            unreachable!()
        };
        signed.body.amount = 1_000;
        assert_eq!(
            Transaction::Transfer(signed).verify_signature(),
            Err(TxError::BadSignature)
        );
    }

    #[test]
    fn pretending_to_be_someone_else_fails() {
        let alice = Wallet::generate();
        let mallory = Wallet::generate();
        let Transaction::Transfer(mut signed) =
            Transaction::transfer(&mallory, &mallory.address(), 10, 1)
        else {
            unreachable!()
        };
        // Mallory 自己簽，卻把付款人寫成 Alice。
        signed.public_key = alice.address();
        assert_eq!(
            Transaction::Transfer(signed).verify_signature(),
            Err(TxError::BadSignature)
        );
    }

    #[test]
    fn ids_differ_by_content_and_json_has_a_type_tag() {
        let alice = Wallet::generate();
        let a = Transaction::transfer(&alice, &alice.address(), 1, 1);
        let b = Transaction::transfer(&alice, &alice.address(), 1, 2);
        assert_ne!(a.id(), b.id());
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["type"], "transfer");
        assert_eq!(json["body"]["amount"], 1);
        let back: Transaction = serde_json::from_value(json).unwrap();
        assert_eq!(back, a);
    }
}
