//! 第 9 章：FileChain 的命令列客戶端，連到真的節點。原本 `SWFilechain/UI_test.ipynb` 的流程：
//! 上傳整個資料夾的檔案 → 看 pool → 出塊 → 看鏈 → 用檔名查詢並還原檔案。
//!
//! 先用 docker compose 起 3 個節點，再執行：
//!
//! ```sh
//! cargo run -p chain-node --example ch09-client -- http://localhost:3001 docs/images/samples Low AMERICA
//! ```
//!
//! 參數依序是：節點網址、資料夾、驗證等級（None／Low／Medium／High）、單位名稱，都可以省略。
//! 跟原本的 notebook 一樣，用檔名當作 `certificateOwner`。

use std::path::PathBuf;

use blockchain_from_scratch::filechain::{CerLevel, FileHit, Upload, prepare_upload, read_hit};
use blockchain_from_scratch::wallet::Wallet;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let node = args
        .next()
        .unwrap_or_else(|| "http://localhost:3001".to_string());
    let folder = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/images/samples")
    });
    let level: CerLevel =
        serde_json::from_value(Value::String(args.next().unwrap_or_else(|| "Low".into())))?;
    let unit_name = args.next().unwrap_or_else(|| "AMERICA".to_string());
    let client = reqwest::Client::new();

    // 這次執行用的單位金鑰。真的單位會把私鑰好好存起來，每次都用同一把。
    let unit = Wallet::generate();
    println!("單位 {unit_name} 的公鑰：{}", unit.address());

    let key: Value = client
        .get(format!("{node}/getPublicKey"))
        .send()
        .await?
        .json()
        .await?;
    let chain_public_key = key["public_key"]
        .as_str()
        .ok_or("沒有 public_key")?
        .to_string();
    println!("鏈的公鑰：{chain_public_key}");

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&folder)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let bytes = std::fs::read(&path)?;
        let upload = Upload {
            file: &bytes,
            certificate_owner: &name,
            cer_level: level,
            unit_name: &unit_name,
            timestamp: chain_node::now_ms(),
        };
        let data = prepare_upload(upload, &unit, &chain_public_key)?;
        let response = client
            .post(format!("{node}/addData"))
            .json(&data)
            .send()
            .await?;
        println!(
            "上傳 {name}（{} bytes）：HTTP {}",
            bytes.len(),
            response.status()
        );
        files.push((name, bytes));
    }

    let pool: Vec<Value> = client
        .get(format!("{node}/poolList"))
        .send()
        .await?
        .json()
        .await?;
    println!("\npool 裡有 {} 筆", pool.len());

    let block: Value = client
        .post(format!("{node}/mineBlock"))
        .send()
        .await?
        .json()
        .await?;
    println!(
        "出塊：{}",
        block
            .get("index")
            .map_or_else(|| block.to_string(), |i| format!("#{i}"))
    );
    let blocks: Vec<Value> = client
        .get(format!("{node}/blocks"))
        .send()
        .await?
        .json()
        .await?;
    println!("鏈上有 {} 個區塊", blocks.len());

    let out_dir = std::env::temp_dir().join("filechain-restored");
    std::fs::create_dir_all(&out_dir)?;
    println!("\n用檔名查詢，還原的檔案放在 {}", out_dir.display());
    for (name, original) in &files {
        let hits: Vec<FileHit> = client
            .get(format!("{node}/queryBlockByName"))
            .query(&[("queryName", name)])
            .send()
            .await?
            .json()
            .await?;
        for hit in &hits {
            let restored = read_hit(hit)?;
            std::fs::write(out_dir.join(name), &restored)?;
            let units: Vec<String> = hit
                .certifications
                .iter()
                .map(|c| format!("{}×{}（{:?}）", c.unit_name, c.count, c.cer_level))
                .collect();
            println!(
                "{name}：跟原檔一樣 {}，驗證單位 {}",
                restored == *original,
                units.join("、")
            );
        }
    }
    Ok(())
}
