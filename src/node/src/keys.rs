//! 鏈的加密金鑰：第一次啟動時產生，之後從檔案讀。
//!
//! 金鑰檔絕對不能 commit 進 git。預設位置 `data/` 已經被 `.gitignore` 擋住；
//! docker compose 把它放在 volume 裡，3 個節點共用同一把。

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use blockchain_from_scratch::filechain::ChainKey;

/// 讀取鏈的金鑰；檔案不存在就產生一把新的並存檔。
///
/// 好幾個節點可能同時啟動、同時發現檔案不存在。為了不讓它們各自產生不同的金鑰，
/// 先寫到暫存檔，再用 hard link 放到正式的檔名：hard link 在檔案已經存在時會失敗，
/// 失敗的那個節點就改讀別人先寫好的檔案。
pub fn load_or_create_chain_key(path: &Path) -> Result<ChainKey> {
    if let Some(key) = read_key(path)? {
        return Ok(key);
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).with_context(|| format!("無法建立資料夾 {}", dir.display()))?;
    }
    let key = ChainKey::generate();
    // 暫存檔的名稱要每次都不同（不同程序、同一個程序的不同執行緒都一樣）。
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    let temp = path.with_extension(format!("tmp-{}-{unique}", std::process::id()));
    write_private(&temp, &key.secret_hex())?;
    let linked = fs::hard_link(&temp, path);
    let _ = fs::remove_file(&temp);
    match linked {
        Ok(()) => {
            tracing::info!(
                "產生新的鏈金鑰：{}（公鑰 {}）",
                path.display(),
                key.public_hex()
            );
            Ok(key)
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            read_key(path)?.with_context(|| format!("{} 剛剛被建立，卻讀不到", path.display()))
        }
        Err(e) => Err(e).with_context(|| format!("無法寫入 {}", path.display())),
    }
}

fn read_key(path: &Path) -> Result<Option<ChainKey>> {
    match fs::read_to_string(path) {
        Ok(text) => ChainKey::from_secret_hex(&text)
            .map(Some)
            .with_context(|| format!("{} 的內容不是鏈的金鑰", path.display())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("無法讀取 {}", path.display())),
    }
}

/// 寫入只有自己（檔案擁有者）能讀的檔案。
fn write_private(path: &Path, contents: &str) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("無法建立 {}", path.display()))?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_once_then_reuses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys/chain-key.hex");
        let first = load_or_create_chain_key(&path).unwrap();
        let second = load_or_create_chain_key(&path).unwrap();
        assert_eq!(first.public_hex(), second.public_hex());
        // 暫存檔不會留下來。
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn concurrent_starts_agree_on_one_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chain-key.hex");
        let keys: Vec<String> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| scope.spawn(|| load_or_create_chain_key(&path).unwrap().public_hex()))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(keys.iter().all(|k| *k == keys[0]), "{keys:?}");
    }

    #[test]
    fn rejects_a_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chain-key.hex");
        fs::write(&path, "not a key").unwrap();
        assert!(load_or_create_chain_key(&path).is_err());
    }
}
