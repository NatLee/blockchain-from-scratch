//! 第 9 章：FileChain（檔案存證）。執行：`cargo run --example ch09-filechain [檔案路徑]`
//!
//! 沒有給檔案時，用 `docs/images/samples/sample-certificate.png`。

use std::path::PathBuf;

use blockchain_from_scratch::filechain::{CerLevel, ChainKey, Upload, prepare_upload, read_hit};
use blockchain_from_scratch::hash::sha256_hex;
use blockchain_from_scratch::node::Node;
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::wallet::Wallet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/images/samples/sample-certificate.png")
        });
    let file = std::fs::read(&path)?;
    println!(
        "檔案：{}（{} bytes，SHA-256 {}…）",
        path.display(),
        file.len(),
        &sha256_hex(&file)[..16]
    );

    // 節點啟動時產生鏈的金鑰；兩個單位各有自己的簽章金鑰。
    let chain_key = ChainKey::generate();
    let mut node = Node::new(
        ChainParams::fixed_difficulty(12),
        Wallet::generate(),
        Some(chain_key.clone()),
    );
    let america = Wallet::generate();
    let china = Wallet::generate();
    println!("鏈的公鑰（GET /getPublicKey）：{}", chain_key.public_hex());

    println!("\n== 1–3. 客戶端：壓縮、用鏈的公鑰加密、用單位的私鑰簽章 ==");
    for (unit, unit_name, level) in [
        (&america, "AMERICA", CerLevel::High),
        (&china, "CHINA", CerLevel::Low),
    ] {
        let upload = Upload {
            file: &file,
            certificate_owner: "Miles",
            cer_level: level,
            unit_name,
            timestamp: 1_700_000_000_000,
        };
        let data = prepare_upload(upload, unit, &chain_key.public_hex())?;
        println!(
            "{unit_name}：上傳 {} 個 base64 字元，簽章 {}…",
            data.base64_data.len(),
            &data.signature[..16]
        );
        // 4–5. 節點驗證簽章，放進 pool（POST /addData）。
        node.add_data(data)?;
    }
    println!("pool 裡有 {} 筆（GET /poolList）", node.mempool().len());

    println!("\n== 還沒出塊時查不到 ==");
    println!("查 Miles：{} 筆", node.query_by_name("Miles")?.len());

    println!("\n== 6. 出塊（POST /mineBlock）==");
    let (block, _) = node.mine_pool(1_700_000_010_000)?;
    println!(
        "區塊 #{} 有 {} 筆交易",
        block.header.index,
        block.data.len()
    );

    println!("\n== 7. 用名稱查詢（GET /queryBlockByName?queryName=Miles）==");
    for hit in node.query_by_name("Miles")? {
        let restored = read_hit(&hit)?;
        println!(
            "檔案 {}…：還原 {} bytes，跟原檔一樣：{}",
            &hit.file_hash[..16],
            restored.len(),
            restored == file
        );
        for c in &hit.certifications {
            println!(
                "  - {}（{:?}）驗證 {} 次",
                c.unit_name, c.cer_level, c.count
            );
        }
    }
    Ok(())
}
