//! 第 5 章（上）：金鑰和數位簽章
//!
//! 區塊鏈上沒有帳號密碼。「這筆錢是我的」靠的是一對金鑰：
//!
//! - **私鑰（secret key）**：只有自己知道，用來簽章。
//! - **公鑰（public key）**：公開給大家，用來驗證簽章。這裡直接把公鑰當成地址。
//!
//! 簽章用 Ed25519（`ed25519-dalek` crate）：金鑰只有 32 bytes、簽章 64 bytes，速度快，
//! 而且沒有「選錯雜湊或填充方式」這類容易出錯的參數。
//! 原本的專案用 RSA PKCS#1 v1.5 加 SHA-1；SHA-1 在 2017 年已經被做出碰撞，不適合再用來簽章。

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

/// 金鑰或簽章的格式不對。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("金鑰要是 64 個十六進位字元")]
    BadKey,
    #[error("簽章要是 128 個十六進位字元")]
    BadSignatureFormat,
    #[error("簽章驗證失敗")]
    VerificationFailed,
}

/// 產生 32 bytes 的隨機數，來源是作業系統（瀏覽器裡是 `crypto.getRandomValues`）。
pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the operating system random number generator is available");
    bytes
}

/// 錢包：一把 Ed25519 私鑰。
///
/// 私鑰不實作 `Debug`、`Serialize`，避免不小心印到 log 或回傳到 API 裡。
#[derive(Clone)]
pub struct Wallet {
    key: SigningKey,
}

impl Wallet {
    /// 用作業系統的隨機數產生新錢包。
    pub fn generate() -> Self {
        Wallet {
            key: SigningKey::from_bytes(&random_bytes()),
        }
    }

    /// 從十六進位的私鑰還原錢包。
    pub fn from_secret_hex(secret_hex: &str) -> Result<Self, KeyError> {
        let mut secret = [0u8; 32];
        hex::decode_to_slice(secret_hex.trim(), &mut secret).map_err(|_| KeyError::BadKey)?;
        Ok(Wallet {
            key: SigningKey::from_bytes(&secret),
        })
    }

    /// 私鑰（十六進位）。只有要存檔時才用得到，千萬不要傳給別人。
    pub fn secret_hex(&self) -> String {
        hex::encode(self.key.to_bytes())
    }

    /// 地址：公鑰的十六進位。
    pub fn address(&self) -> String {
        hex::encode(self.key.verifying_key().to_bytes())
    }

    /// 簽一段訊息，回傳十六進位的簽章。
    pub fn sign(&self, message: &[u8]) -> String {
        hex::encode(self.key.sign(message).to_bytes())
    }
}

/// 地址的格式對不對（64 個十六進位字元，而且是曲線上的點）。
pub fn is_address(address: &str) -> bool {
    parse_public_key(address).is_ok()
}

fn parse_public_key(address: &str) -> Result<VerifyingKey, KeyError> {
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(address, &mut bytes).map_err(|_| KeyError::BadKey)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| KeyError::BadKey)
}

/// 用地址（公鑰）驗證簽章。訊息改了任何一個 byte，或不是這把公鑰簽的，都會失敗。
pub fn verify(address: &str, message: &[u8], signature_hex: &str) -> Result<(), KeyError> {
    let public_key = parse_public_key(address)?;
    let mut bytes = [0u8; 64];
    hex::decode_to_slice(signature_hex, &mut bytes).map_err(|_| KeyError::BadSignatureFormat)?;
    let signature = Signature::from_bytes(&bytes);
    // `verify_strict` 多擋了幾種奇怪的公鑰和簽章，區塊鏈這種要大家算出同樣結果的地方應該用它。
    public_key
        .verify_strict(message, &signature)
        .map_err(|_| KeyError::VerificationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_round_trip() {
        let alice = Wallet::generate();
        let signature = alice.sign(b"pay bob 10");
        assert_eq!(verify(&alice.address(), b"pay bob 10", &signature), Ok(()));
    }

    #[test]
    fn changed_message_fails() {
        let alice = Wallet::generate();
        let signature = alice.sign(b"pay bob 10");
        assert_eq!(
            verify(&alice.address(), b"pay bob 99", &signature),
            Err(KeyError::VerificationFailed)
        );
    }

    #[test]
    fn someone_elses_key_fails() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let signature = alice.sign(b"pay bob 10");
        assert_eq!(
            verify(&bob.address(), b"pay bob 10", &signature),
            Err(KeyError::VerificationFailed)
        );
    }

    #[test]
    fn secret_round_trip() {
        let wallet = Wallet::generate();
        let restored = Wallet::from_secret_hex(&wallet.secret_hex()).unwrap();
        assert_eq!(restored.address(), wallet.address());
        assert!(Wallet::from_secret_hex("xyz").is_err());
    }

    #[test]
    fn rejects_malformed_input() {
        let wallet = Wallet::generate();
        assert!(is_address(&wallet.address()));
        assert!(!is_address("1234"));
        assert_eq!(
            verify(&wallet.address(), b"m", "zz"),
            Err(KeyError::BadSignatureFormat)
        );
    }
}
