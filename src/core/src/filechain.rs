//! 第 9 章：FileChain（檔案存證）
//!
//! 這是 2018 年原本專案的主角。一個單位（例如某個機關）想證明「這個檔案在這個時間點存在，
//! 而且經過我們驗證」，流程是：
//!
//! 1. 客戶端用 gzip 壓縮檔案。
//! 2. 用「鏈的公鑰」加密壓縮後的檔案，只有握有鏈私鑰的節點解得開。
//! 3. 單位用自己的私鑰簽整筆紀錄：名稱、密文、原始檔的雜湊、驗證等級、單位名稱、時間。
//! 4. 上傳到任何一個節點。節點驗證簽章後放進 mempool 並廣播。
//! 5. 出塊之後，用名稱就查得到：節點解密後回傳檔案，並列出驗證過的單位。
//! 6. 客戶端解壓縮，再算一次雜湊，確認跟紀錄裡的 `fileHash` 一樣。
//!
//! 加密用 hybrid encryption（混合加密）：公鑰加密很慢、而且一次只能加密很短的資料，
//! 所以先用公鑰的方法談出一把一次性的對稱金鑰，再用對稱加密處理整個檔案。
//! 原本的專案也是 hybrid：RSA-OAEP 加密一把隨機的 AES 金鑰，再用 AES-EAX 加密檔案。
//! 這裡換成 X25519 金鑰交換 + HKDF + AES-256-GCM，原因見第 9 章的文件。

use std::collections::HashMap;
use std::io::{Read, Write};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::hash::{decode_hash_hex, sha256_hex};
use crate::transaction::{Signed, Transaction, TxBlock, TxError};
use crate::wallet::{Wallet, random_bytes};

/// HKDF 的 info：把這把金鑰綁定在「FileChain 第 1 版」這個用途上，別的用途算不出同一把。
const HKDF_INFO: &[u8] = b"blockchain-from-scratch/filechain/v1";
/// 密文的格式：一次性公鑰（32 bytes）+ nonce（12 bytes）+ AES-GCM 密文和驗證碼。
const EPHEMERAL_LEN: usize = 32;
const NONCE_LEN: usize = 12;

/// FileChain 出錯的原因。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("鏈的公鑰格式不對")]
    BadChainKey,
    #[error("密文太短或格式不對")]
    BadCiphertext,
    #[error("解密失敗：密文被改過，或不是用這條鏈的公鑰加密的")]
    DecryptFailed,
    #[error("base64 格式不對")]
    BadBase64,
    #[error("解壓縮失敗：{0}")]
    BadGzip(String),
    #[error("還原後的檔案雜湊對不上 fileHash")]
    HashMismatch,
    #[error("這個節點沒有鏈的私鑰，不能解密")]
    NoChainKey,
}

/// 驗證等級。值跟原本網頁下拉選單的 value 一樣。
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum CerLevel {
    #[default]
    None,
    Low,
    Medium,
    High,
}

/// 一筆檔案存證的內容（由單位簽章）。欄位名稱沿用原本 API 的寫法。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    /// 檔案的名稱，查詢時用它找。
    #[serde(rename = "certificateOwner")]
    pub certificate_owner: String,
    /// 壓縮、加密後的檔案，base64。
    #[serde(rename = "base64Data")]
    pub base64_data: String,
    /// 原始檔案的 SHA-256。
    #[serde(rename = "fileHash")]
    pub file_hash: String,
    #[serde(rename = "cerLevel")]
    pub cer_level: CerLevel,
    /// 單位自己宣告的名稱。原本的程式把兩把公鑰寫死對應到單位名稱；
    /// 現在由單位簽章宣告，誰簽的就是誰宣告的（要不要相信，見第 9 章的練習）。
    #[serde(rename = "unitName")]
    pub unit_name: String,
    /// 毫秒。同一個單位重複驗證同一個檔案時，讓每筆紀錄的編號不同。
    pub timestamp: u64,
}

impl FileRecord {
    /// 檢查欄位格式（簽章另外檢查）。
    pub fn validate(&self) -> Result<(), TxError> {
        if self.certificate_owner.trim().is_empty() {
            return Err(TxError::BadFile("certificateOwner 不能是空的".into()));
        }
        if decode_hash_hex(&self.file_hash).is_none() {
            return Err(TxError::BadFile("fileHash 要是 64 個十六進位字元".into()));
        }
        let sealed = BASE64
            .decode(&self.base64_data)
            .map_err(|_| TxError::BadFile("base64Data 不是 base64".into()))?;
        if sealed.len() < EPHEMERAL_LEN + NONCE_LEN + 16 {
            return Err(TxError::BadFile(
                "base64Data 太短，不是 FileChain 的密文".into(),
            ));
        }
        Ok(())
    }
}

/// `POST /addData` 收的 JSON。欄位名稱跟原本的 API 一樣（含 `public_key` 的底線寫法），
/// 另外多了 `unitName` 和 `timestamp`，因為現在簽章涵蓋整筆紀錄。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddData {
    #[serde(rename = "base64Data")]
    pub base64_data: String,
    pub signature: String,
    pub public_key: String,
    #[serde(rename = "certificateOwner")]
    pub certificate_owner: String,
    #[serde(rename = "fileHash")]
    pub file_hash: String,
    #[serde(rename = "cerLevel", default)]
    pub cer_level: CerLevel,
    #[serde(rename = "unitName", default)]
    pub unit_name: String,
    #[serde(default)]
    pub timestamp: u64,
}

impl From<Signed<FileRecord>> for AddData {
    fn from(signed: Signed<FileRecord>) -> Self {
        let Signed {
            body,
            public_key,
            signature,
        } = signed;
        AddData {
            base64_data: body.base64_data,
            signature,
            public_key,
            certificate_owner: body.certificate_owner,
            file_hash: body.file_hash,
            cer_level: body.cer_level,
            unit_name: body.unit_name,
            timestamp: body.timestamp,
        }
    }
}

impl From<AddData> for Transaction {
    fn from(data: AddData) -> Self {
        Transaction::File(Signed {
            body: FileRecord {
                certificate_owner: data.certificate_owner,
                base64_data: data.base64_data,
                file_hash: data.file_hash,
                cer_level: data.cer_level,
                unit_name: data.unit_name,
                timestamp: data.timestamp,
            },
            public_key: data.public_key,
            signature: data.signature,
        })
    }
}

/// gzip 壓縮。
pub fn gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .expect("writing into a Vec cannot fail");
    encoder.finish().expect("writing into a Vec cannot fail")
}

/// gzip 解壓縮。
pub fn gunzip(data: &[u8]) -> Result<Vec<u8>, FileError> {
    let mut out = Vec::new();
    GzDecoder::new(data)
        .read_to_end(&mut out)
        .map_err(|e| FileError::BadGzip(e.to_string()))?;
    Ok(out)
}

/// 鏈的加密金鑰（X25519）。同一個網路的節點共用一把，所以從哪個節點都查得到檔案。
#[derive(Clone)]
pub struct ChainKey {
    secret: StaticSecret,
}

impl ChainKey {
    pub fn generate() -> Self {
        ChainKey {
            secret: StaticSecret::from(random_bytes()),
        }
    }

    pub fn from_secret_hex(secret_hex: &str) -> Result<Self, FileError> {
        let bytes = decode_hash_hex(secret_hex.trim()).ok_or(FileError::BadChainKey)?;
        Ok(ChainKey {
            secret: StaticSecret::from(bytes),
        })
    }

    /// 私鑰（十六進位），只用來存檔。
    pub fn secret_hex(&self) -> String {
        hex::encode(self.secret.to_bytes())
    }

    /// 公鑰（十六進位），`GET /getPublicKey` 回傳的就是它。
    pub fn public_hex(&self) -> String {
        hex::encode(PublicKey::from(&self.secret).as_bytes())
    }
}

/// 從金鑰交換的結果導出 AES-256 金鑰。salt 放雙方的公鑰，讓金鑰綁定這一次的交換。
fn derive_key(shared: &[u8; 32], ephemeral: &PublicKey, chain: &PublicKey) -> [u8; 32] {
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(ephemeral.as_bytes());
    salt[32..].copy_from_slice(chain.as_bytes());
    let mut key = [0u8; 32];
    Hkdf::<Sha256>::new(Some(&salt), shared)
        .expand(HKDF_INFO, &mut key)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    key
}

/// 用鏈的公鑰加密（hybrid encryption）。
///
/// 1. 產生一把一次性的 X25519 金鑰，跟鏈的公鑰做金鑰交換，得到只有雙方算得出的共同秘密。
/// 2. 用 HKDF 把共同秘密變成一把 AES-256 金鑰。
/// 3. 用 AES-256-GCM 加密資料；GCM 會附一段驗證碼，密文被改過就解不開。
/// 4. 輸出：一次性公鑰 + nonce + 密文。鏈的私鑰加上一次性公鑰，就能算出同一把 AES 金鑰。
pub fn seal(plaintext: &[u8], chain_public_hex: &str) -> Result<Vec<u8>, FileError> {
    let chain_public =
        PublicKey::from(decode_hash_hex(chain_public_hex).ok_or(FileError::BadChainKey)?);
    let ephemeral = StaticSecret::from(random_bytes());
    let ephemeral_public = PublicKey::from(&ephemeral);
    let shared = ephemeral.diffie_hellman(&chain_public);
    if !shared.was_contributory() {
        // 對方給的「公鑰」是特殊的弱點，交換出來的秘密是全 0。
        return Err(FileError::BadChainKey);
    }
    let key = derive_key(shared.as_bytes(), &ephemeral_public, &chain_public);
    let nonce: [u8; NONCE_LEN] = random_bytes();
    let ciphertext = Aes256Gcm::new(&key.into())
        .encrypt(&Nonce::from(nonce), plaintext)
        .map_err(|_| FileError::DecryptFailed)?;

    let mut out = Vec::with_capacity(EPHEMERAL_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(ephemeral_public.as_bytes());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// 用鏈的私鑰解密 `seal` 的輸出。
pub fn open(sealed: &[u8], chain_key: &ChainKey) -> Result<Vec<u8>, FileError> {
    if sealed.len() < EPHEMERAL_LEN + NONCE_LEN {
        return Err(FileError::BadCiphertext);
    }
    let (ephemeral_bytes, rest) = sealed.split_at(EPHEMERAL_LEN);
    let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);
    let ephemeral_public = PublicKey::from(
        <[u8; EPHEMERAL_LEN]>::try_from(ephemeral_bytes).map_err(|_| FileError::BadCiphertext)?,
    );
    let nonce = <[u8; NONCE_LEN]>::try_from(nonce_bytes).map_err(|_| FileError::BadCiphertext)?;
    let chain_public = PublicKey::from(&chain_key.secret);
    let shared = chain_key.secret.diffie_hellman(&ephemeral_public);
    let key = derive_key(shared.as_bytes(), &ephemeral_public, &chain_public);
    Aes256Gcm::new(&key.into())
        .decrypt(&Nonce::from(nonce), ciphertext)
        .map_err(|_| FileError::DecryptFailed)
}

/// 上傳一個檔案要填的資料。
pub struct Upload<'a> {
    pub file: &'a [u8],
    pub certificate_owner: &'a str,
    pub cer_level: CerLevel,
    pub unit_name: &'a str,
    pub timestamp: u64,
}

/// 客戶端：壓縮、加密、簽章，做出 `POST /addData` 要送的 JSON。
///
/// `Upload<'a>` 裡的 `'a` 是生命週期（lifetime）：表示這些欄位借用的資料至少要活得跟 `upload` 一樣久。
pub fn prepare_upload(
    upload: Upload<'_>,
    unit: &Wallet,
    chain_public_hex: &str,
) -> Result<AddData, FileError> {
    let sealed = seal(&gzip(upload.file), chain_public_hex)?;
    let record = FileRecord {
        certificate_owner: upload.certificate_owner.to_string(),
        base64_data: BASE64.encode(sealed),
        file_hash: sha256_hex(upload.file),
        cer_level: upload.cer_level,
        unit_name: upload.unit_name.to_string(),
        timestamp: upload.timestamp,
    };
    Ok(AddData::from(Signed::sign(record, unit)))
}

/// 一個單位對某個檔案的驗證：同一個單位、同一個等級驗證了幾次。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Certification {
    pub public_key: String,
    pub unit_name: String,
    pub cer_level: CerLevel,
    pub count: u32,
}

/// 鏈上找到的一個檔案（還是密文）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMatch {
    pub file_hash: String,
    /// 第一個（最新的）紀錄裡的密文，base64。
    pub sealed_base64: String,
    pub certifications: Vec<Certification>,
}

/// 依名稱找檔案，規則跟原本的 `query_block_by_name` 一樣：
///
/// - 從最新的區塊、最新的紀錄往回找，名稱（`certificateOwner`）相同的才算。
/// - 依原始檔的雜湊分組：內容一樣的檔案算同一個，列出驗證過的單位和次數。
/// - 驗證等級是 `Low`、`Medium`、`High` 時，每個等級只認「最新」那份檔案：
///   同一個等級較舊的紀錄，只有在那份檔案已經在結果裡時才算進去。等級 `None` 的紀錄全部都算。
///
/// 跟原本不一樣的地方：原本第一次遇到某個等級時，會把那個檔案的單位清單整個重設，
/// 先前算到的單位就不見了。這裡改成合併（見 `tests::first_seen_level_merges_instead_of_resetting`）。
pub fn find_by_name(blocks: &[TxBlock], name: &str) -> Vec<FileMatch> {
    let mut matches: Vec<FileMatch> = Vec::new();
    let mut seen_levels: Vec<CerLevel> = Vec::new();

    let newest_first = blocks
        .iter()
        .rev()
        .flat_map(|block| block.data.iter().rev())
        .filter_map(|tx| match tx {
            Transaction::File(signed) if signed.body.certificate_owner == name => Some(signed),
            _ => None,
        });

    for signed in newest_first {
        let record = &signed.body;
        let position = matches.iter().position(|m| m.file_hash == record.file_hash);
        let keep = match record.cer_level {
            CerLevel::None => true,
            level if seen_levels.contains(&level) => position.is_some(),
            level => {
                seen_levels.push(level);
                true
            }
        };
        if !keep {
            continue;
        }
        let entry = match position {
            Some(i) => &mut matches[i],
            None => {
                matches.push(FileMatch {
                    file_hash: record.file_hash.clone(),
                    sealed_base64: record.base64_data.clone(),
                    certifications: Vec::new(),
                });
                matches.last_mut().expect("just pushed")
            }
        };
        match entry
            .certifications
            .iter_mut()
            .find(|c| c.public_key == signed.public_key && c.cer_level == record.cer_level)
        {
            Some(certification) => certification.count += 1,
            None => entry.certifications.push(Certification {
                public_key: signed.public_key.clone(),
                unit_name: record.unit_name.clone(),
                cer_level: record.cer_level,
                count: 1,
            }),
        }
    }
    matches
}

/// `GET /queryBlockByName` 回傳的一個檔案：節點已經解密，但還是 gzip 壓縮的狀態，由客戶端解壓縮。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHit {
    pub file_hash: String,
    /// 解密後的 gzip 資料，base64。解不開時是 `null`，原因在 `error`。
    pub gzip_data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub certifications: Vec<Certification>,
}

/// 節點端：查名稱，並用鏈的私鑰解密每個檔案。
pub fn query_by_name(blocks: &[TxBlock], name: &str, chain_key: &ChainKey) -> Vec<FileHit> {
    find_by_name(blocks, name)
        .into_iter()
        .map(|m| {
            let opened = BASE64
                .decode(&m.sealed_base64)
                .map_err(|_| FileError::BadBase64)
                .and_then(|sealed| open(&sealed, chain_key));
            let (gzip_data, error) = match opened {
                Ok(bytes) => (Some(BASE64.encode(bytes)), None),
                Err(e) => (None, Some(e.to_string())),
            };
            FileHit {
                file_hash: m.file_hash,
                gzip_data,
                error,
                certifications: m.certifications,
            }
        })
        .collect()
}

/// 客戶端：把查到的檔案解壓縮，並確認雜湊跟 `fileHash` 一樣。
pub fn read_hit(hit: &FileHit) -> Result<Vec<u8>, FileError> {
    let encoded = hit.gzip_data.as_deref().ok_or(FileError::DecryptFailed)?;
    let compressed = BASE64.decode(encoded).map_err(|_| FileError::BadBase64)?;
    let file = gunzip(&compressed)?;
    if sha256_hex(&file) != hit.file_hash {
        return Err(FileError::HashMismatch);
    }
    Ok(file)
}

/// 統計每個單位驗證了幾個檔案，給網頁顯示用。
pub fn units_summary(hits: &[FileHit]) -> HashMap<String, u32> {
    let mut summary = HashMap::new();
    for certification in hits.iter().flat_map(|h| &h.certifications) {
        *summary.entry(certification.unit_name.clone()).or_default() += certification.count;
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Blockchain;
    use crate::params::ChainParams;
    use crate::pow::mine;

    #[test]
    fn gzip_round_trip() {
        let data = b"hello hello hello hello".repeat(10);
        let compressed = gzip(&data);
        assert!(compressed.len() < data.len());
        assert_eq!(gunzip(&compressed).unwrap(), data);
        assert!(matches!(gunzip(b"not gzip"), Err(FileError::BadGzip(_))));
    }

    #[test]
    fn seal_and_open() {
        let key = ChainKey::generate();
        let sealed = seal(b"secret file", &key.public_hex()).unwrap();
        assert_eq!(open(&sealed, &key).unwrap(), b"secret file");
        // 同樣的內容每次加密結果都不一樣（一次性金鑰和 nonce 都是隨機的）。
        assert_ne!(seal(b"secret file", &key.public_hex()).unwrap(), sealed);
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let key = ChainKey::generate();
        let mut sealed = seal(b"secret file", &key.public_hex()).unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert_eq!(open(&sealed, &key), Err(FileError::DecryptFailed));
        assert_eq!(open(&sealed[..10], &key), Err(FileError::BadCiphertext));
    }

    #[test]
    fn another_chain_cannot_decrypt() {
        let ours = ChainKey::generate();
        let theirs = ChainKey::generate();
        let sealed = seal(b"secret file", &ours.public_hex()).unwrap();
        assert_eq!(open(&sealed, &theirs), Err(FileError::DecryptFailed));
    }

    #[test]
    fn chain_key_round_trip() {
        let key = ChainKey::generate();
        let restored = ChainKey::from_secret_hex(&key.secret_hex()).unwrap();
        assert_eq!(restored.public_hex(), key.public_hex());
        assert!(seal(b"x", "nope").is_err());
    }

    #[test]
    fn prepared_upload_is_signed_and_valid() {
        let unit = Wallet::generate();
        let key = ChainKey::generate();
        let upload = Upload {
            file: b"png bytes",
            certificate_owner: "Miles",
            cer_level: CerLevel::High,
            unit_name: "AMERICA",
            timestamp: 1,
        };
        let data = prepare_upload(upload, &unit, &key.public_hex()).unwrap();
        assert_eq!(data.public_key, unit.address());
        let tx = Transaction::from(data.clone());
        assert_eq!(tx.verify_signature(), Ok(()));
        let Transaction::File(signed) = &tx else {
            unreachable!()
        };
        assert_eq!(signed.body.validate(), Ok(()));

        // 中途把名稱改掉：簽章就對不上了（原本的程式只簽了密文，名稱可以被偷改）。
        let mut forged = data;
        forged.certificate_owner = "Someone else".into();
        assert_eq!(
            Transaction::from(forged).verify_signature(),
            Err(TxError::BadSignature)
        );
    }

    #[test]
    fn add_data_json_uses_the_original_field_names() {
        let json = r#"{"base64Data":"x","signature":"s","public_key":"p","certificateOwner":"Miles","fileHash":"h","cerLevel":"Low"}"#;
        let data: AddData = serde_json::from_str(json).unwrap();
        assert_eq!(data.certificate_owner, "Miles");
        assert_eq!(data.cer_level, CerLevel::Low);
        assert_eq!(data.unit_name, "");
    }

    /// 一條鏈，每筆紀錄各占一個區塊，`records` 從舊到新排。
    fn chain_with(records: &[(&Wallet, &str, &[u8], CerLevel)], key: &ChainKey) -> Vec<TxBlock> {
        let params = ChainParams::fixed_difficulty(0);
        let miner = Wallet::generate();
        let mut chain: Blockchain<Vec<Transaction>> = Blockchain::new(params.clone());
        for (i, (unit, name, file, level)) in records.iter().enumerate() {
            let upload = Upload {
                file,
                certificate_owner: name,
                cer_level: *level,
                unit_name: "unit",
                timestamp: i as u64,
            };
            let tx = Transaction::from(prepare_upload(upload, unit, &key.public_hex()).unwrap());
            let height = chain.latest().header.index + 1;
            let data = vec![
                Transaction::coinbase(&miner.address(), params.block_reward, height),
                tx,
            ];
            let (block, _) = mine(chain.next_block(1_700_000_000_000 + i as u64, data));
            chain.push(block).unwrap();
        }
        chain.blocks().to_vec()
    }

    #[test]
    fn groups_by_file_hash_and_counts_units() {
        let key = ChainKey::generate();
        let a = Wallet::generate();
        let b = Wallet::generate();
        let blocks = chain_with(
            &[
                (&a, "Miles", b"v1", CerLevel::None),
                (&a, "Miles", b"v1", CerLevel::None),
                (&b, "Miles", b"v1", CerLevel::None),
                (&b, "Other", b"zz", CerLevel::None),
            ],
            &key,
        );
        let found = find_by_name(&blocks, "Miles");
        assert_eq!(found.len(), 1);
        let counts: Vec<(String, u32)> = found[0]
            .certifications
            .iter()
            .map(|c| (c.public_key.clone(), c.count))
            .collect();
        assert_eq!(counts, vec![(b.address(), 1), (a.address(), 2)]);

        let hits = query_by_name(&blocks, "Miles", &key);
        assert_eq!(read_hit(&hits[0]).unwrap(), b"v1");
        assert_eq!(units_summary(&hits)["unit"], 3);
    }

    #[test]
    fn each_level_keeps_only_its_newest_file() {
        let key = ChainKey::generate();
        let a = Wallet::generate();
        let b = Wallet::generate();
        // 從舊到新：Low 先驗證 v1，之後 Low 又驗證了 v2（較新）。
        let blocks = chain_with(
            &[
                (&a, "Miles", b"v1", CerLevel::Low),
                (&b, "Miles", b"v2", CerLevel::Low),
            ],
            &key,
        );
        let found = find_by_name(&blocks, "Miles");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file_hash, sha256_hex(b"v2"));
    }

    #[test]
    fn older_records_still_count_when_their_file_is_already_listed() {
        let key = ChainKey::generate();
        let a = Wallet::generate();
        let b = Wallet::generate();
        let blocks = chain_with(
            &[
                (&a, "Miles", b"v1", CerLevel::Low),
                (&b, "Miles", b"v1", CerLevel::Low),
                (&b, "Miles", b"v1", CerLevel::None),
            ],
            &key,
        );
        let found = find_by_name(&blocks, "Miles");
        assert_eq!(found.len(), 1);
        let total: u32 = found[0].certifications.iter().map(|c| c.count).sum();
        assert_eq!(total, 3);
    }

    /// 回歸測試：原本的程式第一次遇到某個等級時，會把那個檔案的單位清單整個重設。
    /// 這個情境在原本的程式只會剩下 1 個單位，現在 2 個都在。
    #[test]
    fn first_seen_level_merges_instead_of_resetting() {
        let key = ChainKey::generate();
        let a = Wallet::generate();
        let b = Wallet::generate();
        // 從舊到新：A 用 Low 驗證 v1，B 用 Medium 驗證 v1。
        let blocks = chain_with(
            &[
                (&a, "Miles", b"v1", CerLevel::Low),
                (&b, "Miles", b"v1", CerLevel::Medium),
            ],
            &key,
        );
        let found = find_by_name(&blocks, "Miles");
        assert_eq!(found.len(), 1);
        let units: Vec<&str> = found[0]
            .certifications
            .iter()
            .map(|c| c.public_key.as_str())
            .collect();
        assert_eq!(units, vec![b.address().as_str(), a.address().as_str()]);
    }

    #[test]
    fn read_hit_detects_a_wrong_hash() {
        let hit = FileHit {
            file_hash: sha256_hex(b"something else"),
            gzip_data: Some(BASE64.encode(gzip(b"file"))),
            error: None,
            certifications: vec![],
        };
        assert_eq!(read_hit(&hit), Err(FileError::HashMismatch));
    }
}
