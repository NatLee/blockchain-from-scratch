//! 多節點整合測試：在本機開 3 個真的節點（隨機 port），用 HTTP API 和 websocket 互相連線。

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use blockchain_from_scratch::filechain::{
    CerLevel, ChainKey, FileHit, Upload, prepare_upload, read_hit,
};
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::wallet::Wallet;
use chain_node::{AppState, peers, serve};
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// 開一個節點，回傳它的 HTTP 網址（例如 `http://127.0.0.1:54321`）。所有節點共用同一把鏈金鑰。
async fn start_node(chain_key: &ChainKey) -> (String, Arc<AppState>) {
    let state = AppState::new(ChainParams::fixed_difficulty(6), chain_key.clone());
    // port 0：讓作業系統挑一個沒人用的 port，測試才不會跟別的程式撞到。
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, state.clone()));
    (format!("http://{address}"), state)
}

fn ws(url: &str) -> String {
    url.replacen("http://", "ws://", 1)
}

async fn get(client: &reqwest::Client, url: &str) -> Value {
    client.get(url).send().await.unwrap().json().await.unwrap()
}

async fn post(client: &reqwest::Client, url: &str, body: Value) -> Value {
    client
        .post(url)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// 每 50 毫秒檢查一次，最多等 10 秒。
async fn eventually<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    for _ in 0..200 {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("等不到：{what}");
}

#[tokio::test]
async fn three_nodes_share_files_and_blocks() {
    let chain_key = ChainKey::generate();
    let (n1, _) = start_node(&chain_key).await;
    let (n2, _) = start_node(&chain_key).await;
    let (n3, s3) = start_node(&chain_key).await;
    let client = reqwest::Client::new();

    // 節點 2 透過 API 連到節點 1；節點 3 用程式直接連到 1 和 2（跟啟動時的 PEERS 一樣）。
    assert_eq!(
        post(&client, &format!("{n2}/addPeer"), json!({"peer": ws(&n1)})).await,
        json!({"status": true})
    );
    peers::connect(s3.clone(), ws(&n1)).await.unwrap();
    peers::connect(s3, ws(&n2)).await.unwrap();
    eventually("節點 1 有兩個連線", || async {
        get(&client, &format!("{n1}/peers"))
            .await
            .as_array()
            .unwrap()
            .len()
            == 2
    })
    .await;

    // 從節點 1 上傳一個檔案。
    let public_key = get(&client, &format!("{n1}/getPublicKey")).await["public_key"]
        .as_str()
        .unwrap()
        .to_string();
    let file = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/images/samples/sample-sunrise.png"
    ))
    .unwrap();
    let upload = Upload {
        file: &file,
        certificate_owner: "Miles",
        cer_level: CerLevel::Low,
        unit_name: "AMERICA",
        timestamp: 1_700_000_000_000,
    };
    let data = prepare_upload(upload, &Wallet::generate(), &public_key).unwrap();
    post(
        &client,
        &format!("{n1}/addData"),
        serde_json::to_value(&data).unwrap(),
    )
    .await;

    eventually("檔案廣播到節點 3 的 pool", || async {
        get(&client, &format!("{n3}/poolList"))
            .await
            .as_array()
            .unwrap()
            .len()
            == 1
    })
    .await;

    // 節點 2 出塊。
    let block = post(&client, &format!("{n2}/mineBlock"), json!({})).await;
    assert_eq!(block["index"], 1);
    for node in [&n1, &n3] {
        eventually("區塊傳到其他節點", || async {
            get(&client, &format!("{node}/blocks"))
                .await
                .as_array()
                .unwrap()
                .len()
                == 2
        })
        .await;
    }
    eventually("其他節點的 pool 清空", || async {
        get(&client, &format!("{n3}/poolList")).await == json!([])
    })
    .await;

    // 從節點 3 查詢，還原的檔案跟原本一樣。
    let hits: Vec<FileHit> = serde_json::from_value(
        get(&client, &format!("{n3}/queryBlockByName?queryName=Miles")).await,
    )
    .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(read_hit(&hits[0]).unwrap(), file);

    // 三個節點的最新區塊一樣。
    let mut tips = Vec::new();
    for node in [&n1, &n2, &n3] {
        tips.push(get(&client, &format!("{node}/")).await["latestHash"].clone());
    }
    assert!(tips.iter().all(|t| *t == tips[0]), "{tips:?}");
}

#[tokio::test]
async fn a_late_node_catches_up() {
    let chain_key = ChainKey::generate();
    let (n1, s1) = start_node(&chain_key).await;
    let client = reqwest::Client::new();

    // 節點 1 先自己挖兩個區塊（用 core 的 API 放一筆紀錄再出塊）。
    for i in 0..2 {
        let public_key = chain_key.public_hex();
        let upload = Upload {
            file: b"x",
            certificate_owner: "Late",
            cer_level: CerLevel::None,
            unit_name: "unit",
            timestamp: i,
        };
        let data = prepare_upload(upload, &Wallet::generate(), &public_key).unwrap();
        post(
            &client,
            &format!("{n1}/addData"),
            serde_json::to_value(&data).unwrap(),
        )
        .await;
        post(&client, &format!("{n1}/mineBlock"), json!({})).await;
    }

    // 節點 2 晚加入：連上之後先問最新區塊，發現落後，再要整條鏈。
    let (n2, s2) = start_node(&chain_key).await;
    peers::connect(s2, ws(&n1)).await.unwrap();
    eventually("節點 2 補齊整條鏈", || async {
        get(&client, &format!("{n2}/blocks"))
            .await
            .as_array()
            .unwrap()
            .len()
            == 3
    })
    .await;
    assert_eq!(s1.peer_addresses().len(), 1);
}
