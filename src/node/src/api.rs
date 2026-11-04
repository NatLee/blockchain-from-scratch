//! HTTP API。路徑和參數沿用原本的專案：
//!
//! | 方法 | 路徑 | 做什麼 |
//! |---|---|---|
//! | GET | `/blocks` | 整條鏈 |
//! | POST | `/mineBlock` | 把 pool 打包成新區塊（pool 是空的就不出塊） |
//! | GET | `/peers` | 連線中的節點 |
//! | POST | `/addPeer` | 連到新的節點：`{"peer": "ws://localhost:3002"}` |
//! | POST | `/addData` | FileChain：上傳一筆檔案存證 |
//! | GET | `/queryBlockByName?queryName=…` | FileChain：用名稱查檔案 |
//! | GET | `/poolList` | 還沒上鏈的交易（mempool） |
//! | GET | `/getPublicKey` | FileChain：鏈的加密公鑰 |
//!
//! 新增的：`POST /addTransaction`（任何簽好的交易，例如轉帳）、`GET /balances`（餘額），
//! 以及 `GET /`：一般請求回傳節點資訊，websocket 請求就升級成節點之間的連線（跟原本一樣用 `/`）。

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message as WsMessage, WebSocketUpgrade};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use blockchain_from_scratch::filechain::AddData;
use blockchain_from_scratch::node::{MineError, PublicKeyResponse, StatusResponse};
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::pow;
use blockchain_from_scratch::transaction::{Transaction, TxError};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::config::{MAX_P2P_MESSAGE_BYTES, MAX_REQUEST_BYTES};
use crate::{AppState, now_ms, peers};

type AppJson = Response;

/// 組出所有路由。
pub fn router(state: Arc<AppState>) -> Router {
    // 任何網頁都可以呼叫這個節點的 API（包括 GitHub Pages 上的教材網頁連到你電腦上的節點）。
    // 這是學習用的節點，沒有登入；不要把它開在公開的網路上。
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_private_network(true);
    Router::new()
        .route("/", get(root))
        .route("/blocks", get(blocks))
        .route("/mineBlock", post(mine_block))
        .route("/peers", get(list_peers))
        .route("/addPeer", post(add_peer))
        .route("/addData", post(add_data))
        .route("/addTransaction", post(add_transaction))
        .route("/queryBlockByName", get(query_block_by_name))
        .route("/poolList", get(pool_list))
        .route("/getPublicKey", get(get_public_key))
        .route("/balances", get(balances))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .layer(cors)
        .with_state(state)
}

/// 失敗時回傳原本 API 的 `{"status": false, "message": …}` 格式。
fn failure(code: StatusCode, message: impl ToString) -> AppJson {
    let body = StatusResponse {
        status: false,
        message: Some(message.to_string()),
    };
    (code, Json(body)).into_response()
}

/// `GET /` 回傳的節點資訊。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeInfo {
    name: &'static str,
    address: String,
    height: u64,
    latest_hash: String,
    next_difficulty: u32,
    total_work: String,
    pool_size: usize,
    peers: Vec<String>,
    params: ChainParams,
}

async fn root(
    State(state): State<Arc<AppState>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> AppJson {
    match upgrade {
        // 別的節點連進來：升級成 websocket。
        Ok(ws) => ws
            .max_message_size(MAX_P2P_MESSAGE_BYTES)
            .max_frame_size(MAX_P2P_MESSAGE_BYTES)
            .on_upgrade(move |socket| async move {
                let (sink, stream) = socket.split();
                let incoming = stream.filter_map(|frame| async move {
                    match frame {
                        Ok(WsMessage::Text(text)) => Some(text.as_str().to_owned()),
                        _ => None,
                    }
                });
                let outgoing = sink.with(|text: String| async move {
                    Ok::<_, axum::Error>(WsMessage::Text(text.into()))
                });
                peers::run_peer(state, remote.to_string(), incoming, outgoing).await;
            }),
        // 一般的 HTTP 請求：回傳節點資訊。
        Err(_) => {
            let peers = state.peer_addresses();
            let info = state.with_node(|node| {
                let chain = node.chain();
                NodeInfo {
                    name: "blockchain-from-scratch node",
                    address: node.address(),
                    height: chain.latest().header.index,
                    latest_hash: chain.latest().hash.clone(),
                    next_difficulty: chain.next_difficulty(),
                    // u128 超過 JSON 數字能精確表示的範圍，用字串。
                    total_work: chain.total_work().to_string(),
                    pool_size: node.mempool().len(),
                    peers,
                    params: chain.params().clone(),
                }
            });
            Json(info).into_response()
        }
    }
}

async fn blocks(State(state): State<Arc<AppState>>) -> AppJson {
    Json(state.with_node(|node| node.blocks().to_vec())).into_response()
}

async fn pool_list(State(state): State<Arc<AppState>>) -> AppJson {
    Json(state.with_node(|node| node.mempool().transactions().to_vec())).into_response()
}

async fn balances(State(state): State<Arc<AppState>>) -> AppJson {
    Json(state.with_node(|node| node.ledger().balances().clone())).into_response()
}

async fn get_public_key(State(state): State<Arc<AppState>>) -> AppJson {
    match state.with_node(|node| node.chain_public_key()) {
        Some(public_key) => Json(PublicKeyResponse { public_key }).into_response(),
        None => failure(StatusCode::SERVICE_UNAVAILABLE, "這個節點沒有鏈的金鑰"),
    }
}

async fn list_peers(State(state): State<Arc<AppState>>) -> AppJson {
    Json(state.peer_addresses()).into_response()
}

#[derive(Deserialize)]
struct AddPeer {
    peer: String,
}

async fn add_peer(State(state): State<Arc<AppState>>, Json(body): Json<AddPeer>) -> AppJson {
    if !(body.peer.starts_with("ws://") || body.peer.starts_with("wss://")) {
        return failure(
            StatusCode::BAD_REQUEST,
            "peer 要是 ws:// 或 wss:// 開頭的網址",
        );
    }
    // 跟原本一樣：在背景連線，馬上回傳。連線結果看 GET /peers。
    tokio::spawn(async move {
        if let Err(e) = peers::connect(state, body.peer).await {
            tracing::warn!("{e:#}");
        }
    });
    Json(StatusResponse {
        status: true,
        message: None,
    })
    .into_response()
}

/// 收一筆交易：檢查、放進 mempool、廣播，回傳整個 pool（原本 `/addData` 就是回傳 pool）。
fn submit(state: &AppState, tx: Transaction) -> AppJson {
    let result = state.with_node(|node| {
        node.submit(tx)
            .map(|outbound| (outbound, node.mempool().transactions().to_vec()))
    });
    match result {
        Ok((outbound, pool)) => {
            state.dispatch(None, outbound);
            Json(pool).into_response()
        }
        Err(e @ TxError::InsufficientFunds { .. }) => failure(StatusCode::UNPROCESSABLE_ENTITY, e),
        Err(e) => failure(StatusCode::BAD_REQUEST, e),
    }
}

async fn add_data(State(state): State<Arc<AppState>>, Json(data): Json<AddData>) -> AppJson {
    submit(&state, Transaction::from(data))
}

async fn add_transaction(
    State(state): State<Arc<AppState>>,
    Json(tx): Json<Transaction>,
) -> AppJson {
    submit(&state, tx)
}

/// 出塊。挖礦很花時間，所以：先在鎖裡準備好區塊 → 放開鎖、在另一個執行緒挖礦 → 再鎖住把區塊接上。
/// 挖礦期間別的請求和其他節點的訊息照常處理；如果這段時間鏈被別人的區塊更新了，就回 409。
async fn mine_block(State(state): State<Arc<AppState>>) -> AppJson {
    let template = match state.with_node(|node| node.pool_template(now_ms())) {
        Ok(template) => template,
        // 原本的 API 在 pool 是空的時候回 200 加 status false，這裡照舊。
        Err(e @ MineError::EmptyPool) => return failure(StatusCode::OK, e),
        Err(e) => return failure(StatusCode::CONFLICT, e),
    };
    // `spawn_blocking`：挖礦是一直算、不會停下來等的工作，放到專門的執行緒，免得卡住其他 async task。
    let Ok((block, _attempts)) = tokio::task::spawn_blocking(move || pow::mine(template)).await
    else {
        return failure(StatusCode::INTERNAL_SERVER_ERROR, "挖礦的執行緒意外結束");
    };
    match state.with_node(|node| node.accept_mined(block.clone())) {
        Ok(outbound) => {
            state.dispatch(None, outbound);
            Json(block).into_response()
        }
        Err(e) => failure(StatusCode::CONFLICT, MineError::Stale(e)),
    }
}

/// `GET /queryBlockByName?queryName=Miles`。跟原本一樣，可以給好幾個 `queryName`。
async fn query_block_by_name(
    State(state): State<Arc<AppState>>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> AppJson {
    let names: Vec<String> = pairs
        .into_iter()
        .filter(|(key, _)| key == "queryName")
        .map(|(_, value)| value)
        .collect();
    if names.is_empty() {
        return failure(StatusCode::BAD_REQUEST, "缺少 queryName 參數");
    }
    let result = state.with_node(|node| {
        names
            .iter()
            .map(|name| node.query_by_name(name))
            .collect::<Result<Vec<_>, _>>()
    });
    match result {
        Ok(hits) => Json(hits.into_iter().flatten().collect::<Vec<_>>()).into_response(),
        Err(e) => failure(StatusCode::SERVICE_UNAVAILABLE, e),
    }
}
