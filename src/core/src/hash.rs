//! 第 1 章：雜湊（hash）
//!
//! 雜湊函式把任意長度的資料變成固定長度的「指紋」。區塊鏈用的 SHA-256 有三個重要性質：
//!
//! 1. 同樣的輸入，永遠得到同樣的輸出。
//! 2. 輸入只改一點點，輸出就面目全非（雪崩效應）。
//! 3. 從輸出幾乎不可能反推輸入，也幾乎找不到兩個不同的輸入有同樣的輸出。
//!
//! 後面每一章都靠這三個性質：區塊用雜湊串起來（第 3 章）、挖礦是在找特定樣子的雜湊（第 4 章）、
//! 交易的編號也是雜湊（第 5 章）。

use sha2::{Digest, Sha256};

/// SHA-256 的輸出長度：32 bytes，也就是 256 bits。
pub const HASH_LEN: usize = 32;

/// 算出 SHA-256，回傳 32 bytes。
///
/// 參數 `data: &[u8]` 是「借用」一段 bytes：函式只讀不改，也不會拿走呼叫者的資料，
/// 所以呼叫完之後，呼叫者還能繼續用自己的資料。
pub fn sha256(data: &[u8]) -> [u8; HASH_LEN] {
    Sha256::digest(data).into()
}

/// 算出 SHA-256，用 64 個十六進位字元表示（區塊和交易裡存的都是這種格式）。
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha256(data))
}

/// 把 64 個十六進位字元轉回 32 bytes；格式不對就回傳 `None`。
pub fn decode_hash_hex(hash_hex: &str) -> Option<[u8; HASH_LEN]> {
    let mut bytes = [0u8; HASH_LEN];
    hex::decode_to_slice(hash_hex, &mut bytes).ok()?;
    Some(bytes)
}

/// 兩段 bytes 有幾個 bit 不一樣。
///
/// 用來觀察雪崩效應：輸入改一個字，SHA-256 的 256 個 bit 平均會有一半（約 128 個）翻轉。
pub fn bit_difference(a: &[u8], b: &[u8]) -> u32 {
    // `zip` 把兩邊一個一個配對；`^`（XOR）在兩個 bit 不同時得到 1，`count_ones` 數有幾個 1。
    a.iter().zip(b).map(|(x, y)| (x ^ y).count_ones()).sum()
}

/// 開頭有幾個 0 bit。第 4 章的工作量證明用它當作難度的單位。
pub fn leading_zero_bits(hash: &[u8]) -> u32 {
    let mut bits = 0;
    for byte in hash {
        if *byte == 0 {
            bits += 8;
        } else {
            bits += byte.leading_zeros();
            break;
        }
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_published_test_vectors() {
        // FIPS 180-2 附錄的測試向量。
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn same_input_same_output() {
        assert_eq!(sha256(b"blockchain"), sha256(b"blockchain"));
    }

    #[test]
    fn one_character_changes_about_half_the_bits() {
        let a = sha256(b"hello");
        let b = sha256(b"hellp");
        let diff = bit_difference(&a, &b);
        // 不會剛好 128，但一定離 0 和 256 都很遠。
        assert!((80..=176).contains(&diff), "diff = {diff}");
    }

    #[test]
    fn counts_leading_zero_bits() {
        assert_eq!(leading_zero_bits(&[0xff]), 0);
        assert_eq!(leading_zero_bits(&[0x00, 0x0f]), 12);
        assert_eq!(leading_zero_bits(&[0x00, 0x00]), 16);
        assert_eq!(leading_zero_bits(&[0x01]), 7);
    }

    #[test]
    fn decodes_hex_hashes() {
        let hash = sha256(b"abc");
        assert_eq!(decode_hash_hex(&hex::encode(hash)), Some(hash));
        assert_eq!(decode_hash_hex("not hex"), None);
        assert_eq!(decode_hash_hex("abcd"), None);
    }
}
