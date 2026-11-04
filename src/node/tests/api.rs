//! 每個 HTTP API 的測試（不開真的 port，直接把請求交給 router）。

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use blockchain_from_scratch::filechain::{
    CerLevel, ChainKey, FileHit, Upload, prepare_upload, read_hit,
};
use blockchain_from_scratch::node::EMPTY_POOL_MESSAGE;
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::transaction::{Transaction, TxBlock};
use blockchain_from_scratch::wallet::Wallet;
use chain_node::{AppState, api};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

fn app() -> Router {
    let state: Arc<AppState> =
        AppState::new(ChainParams::fixed_difficulty(4), ChainKey::generate());
    api::router(state).layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))))
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(json) => request
            .header("content-type", "application/json")
            .body(Body::from(json.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    // `clone()` 一個 Router 很便宜：裡面的狀態是 `Arc`，兩邊指向同一份。
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn chain_public_key(app: &Router) -> String {
    let (status, body) = call(app, "GET", "/getPublicKey", None).await;
    assert_eq!(status, StatusCode::OK);
    body["public_key"].as_str().unwrap().to_string()
}

fn upload(file: &[u8], level: CerLevel) -> Upload<'_> {
    Upload {
        file,
        certificate_owner: "Miles",
        cer_level: level,
        unit_name: "AMERICA",
        timestamp: 1_700_000_000_000,
    }
}

#[tokio::test]
async fn get_public_key_returns_a_hex_key() {
    let app = app();
    let key = chain_public_key(&app).await;
    assert_eq!(key.len(), 64);
    assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn filechain_flow_add_mine_query() {
    let app = app();
    let unit = Wallet::generate();
    let file = b"a tiny file".to_vec();
    let data = prepare_upload(
        upload(&file, CerLevel::High),
        &unit,
        &chain_public_key(&app).await,
    )
    .unwrap();

    let (status, pool) = call(
        &app,
        "POST",
        "/addData",
        Some(serde_json::to_value(&data).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pool.as_array().unwrap().len(), 1);
    assert_eq!(pool[0]["type"], "file");

    let (_, pool) = call(&app, "GET", "/poolList", None).await;
    assert_eq!(pool.as_array().unwrap().len(), 1);

    // 還沒出塊：查不到。
    let (status, hits) = call(&app, "GET", "/queryBlockByName?queryName=Miles", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hits, json!([]));

    let (status, block) = call(&app, "POST", "/mineBlock", Some(json!({"data": ""}))).await;
    assert_eq!(status, StatusCode::OK);
    let block: TxBlock = serde_json::from_value(block).unwrap();
    assert_eq!(block.header.index, 1);
    assert_eq!(block.data.len(), 2);

    let (_, pool) = call(&app, "GET", "/poolList", None).await;
    assert_eq!(pool, json!([]));
    let (_, blocks) = call(&app, "GET", "/blocks", None).await;
    assert_eq!(blocks.as_array().unwrap().len(), 2);

    let (_, hits) = call(
        &app,
        "GET",
        "/queryBlockByName?queryName=Miles&queryName=Nobody",
        None,
    )
    .await;
    let hits: Vec<FileHit> = serde_json::from_value(hits).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(read_hit(&hits[0]).unwrap(), file);
    assert_eq!(hits[0].certifications[0].public_key, unit.address());
    assert_eq!(hits[0].certifications[0].cer_level, CerLevel::High);
}

#[tokio::test]
async fn mining_an_empty_pool_keeps_the_original_answer() {
    let (status, body) = call(&app(), "POST", "/mineBlock", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"status": false, "message": EMPTY_POOL_MESSAGE})
    );
}

#[tokio::test]
async fn add_data_rejects_a_bad_signature() {
    let app = app();
    let mut data = prepare_upload(
        upload(b"file", CerLevel::None),
        &Wallet::generate(),
        &chain_public_key(&app).await,
    )
    .unwrap();
    data.certificate_owner = "Someone else".into();
    let (status, body) = call(
        &app,
        "POST",
        "/addData",
        Some(serde_json::to_value(&data).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["status"], false);
    assert!(body["message"].as_str().unwrap().contains("簽章"));
}

#[tokio::test]
async fn add_data_rejects_malformed_json() {
    let (status, _) = call(&app(), "POST", "/addData", Some(json!({"base64Data": "x"}))).await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn transfers_need_money_and_balances_show_rewards() {
    let app = app();
    let alice = Wallet::generate();
    let tx = Transaction::transfer(&alice, &Wallet::generate().address(), 5, 1);
    let (status, body) = call(
        &app,
        "POST",
        "/addTransaction",
        Some(serde_json::to_value(&tx).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["message"].as_str().unwrap().contains("餘額不足"));

    // 放一筆檔案紀錄進 pool，挖一個區塊，礦工拿到出塊獎勵。
    let data = prepare_upload(
        upload(b"file", CerLevel::None),
        &alice,
        &chain_public_key(&app).await,
    )
    .unwrap();
    call(
        &app,
        "POST",
        "/addData",
        Some(serde_json::to_value(&data).unwrap()),
    )
    .await;
    call(&app, "POST", "/mineBlock", None).await;
    let (_, info) = call(&app, "GET", "/", None).await;
    let miner = info["address"].as_str().unwrap();
    let (_, balances) = call(&app, "GET", "/balances", None).await;
    assert_eq!(balances[miner], 50);
    assert_eq!(info["height"], 1);
}

#[tokio::test]
async fn add_peer_validates_the_url_and_peers_starts_empty() {
    let app = app();
    let (status, _) = call(
        &app,
        "POST",
        "/addPeer",
        Some(json!({"peer": "http://example.com"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = call(
        &app,
        "POST",
        "/addPeer",
        Some(json!({"peer": "ws://127.0.0.1:9"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"status": true}));
    let (_, peers) = call(&app, "GET", "/peers", None).await;
    assert_eq!(peers, json!([]));
}

#[tokio::test]
async fn query_needs_a_name() {
    let (status, body) = call(&app(), "GET", "/queryBlockByName", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["status"], false);
}

#[tokio::test]
async fn cors_allows_browser_pages() {
    let request = Request::builder()
        .method("OPTIONS")
        .uri("/blocks")
        .header("origin", "https://natlee.github.io")
        .header("access-control-request-method", "GET")
        .header("access-control-request-private-network", "true")
        .body(Body::empty())
        .unwrap();
    let response = app().oneshot(request).await.unwrap();
    let headers = response.headers();
    assert_eq!(headers["access-control-allow-origin"], "*");
    assert_eq!(headers["access-control-allow-private-network"], "true");
}
