//! 區塊鏈節點程式：把 `blockchain_from_scratch::node::Node` 接上真的網路。
//!
//! - [`api`]：HTTP API（axum），原本的 8 個 API 都在。
//! - [`peers`]：節點之間的 websocket 連線。
//! - [`AppState`]：大家共用的狀態。
//!
//! 節點的邏輯全部在 core 裡（不碰網路）；這個 crate 只負責「收訊息 → 交給 Node → 把結果送出去」。

pub mod api;
pub mod config;
pub mod keys;
pub mod peers;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use blockchain_from_scratch::filechain::ChainKey;
use blockchain_from_scratch::node::{Node, Outbound};
use blockchain_from_scratch::p2p::Message;
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::wallet::Wallet;
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;

pub use config::Config;

/// 現在時間（毫秒）。core 不讀時鐘，所以由節點程式提供。
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 一個連線中的節點。
struct Peer {
    address: String,
    /// 要送給這個節點的訊息丟進這個 channel，由負責這條連線的 task 寫到 websocket。
    sender: UnboundedSender<String>,
}

/// 所有 HTTP handler 和 websocket 連線共用的狀態。
///
/// 會被很多 task 同時用到，所以外面包 `Arc`（多個擁有者共用同一份資料），
/// 裡面需要修改的部分包 `Mutex`（同一時間只有一個 task 能改）。
/// 這裡用標準庫的 `std::sync::Mutex`，因為鎖住的期間不會 `.await`：
/// 拿到鎖、算完、放開，不會讓其他 task 等太久。
pub struct AppState {
    node: Mutex<Node>,
    peers: Mutex<BTreeMap<u64, Peer>>,
    next_peer_id: AtomicU64,
}

impl AppState {
    pub fn new(params: ChainParams, chain_key: ChainKey) -> Arc<Self> {
        let wallet = Wallet::generate();
        tracing::info!("礦工地址（出塊獎勵會給這個地址）：{}", wallet.address());
        Arc::new(AppState {
            node: Mutex::new(Node::new(params, wallet, Some(chain_key))),
            peers: Mutex::new(BTreeMap::new()),
            next_peer_id: AtomicU64::new(1),
        })
    }

    /// 鎖住節點做一件事，做完把節點產生的事件寫到 log。
    ///
    /// `f: impl FnOnce(&mut Node) -> R`：傳進來一個函式，它會拿到節點的可變借用；
    /// 函式結束時借用也結束，鎖就跟著放開。
    pub fn with_node<R>(&self, f: impl FnOnce(&mut Node) -> R) -> R {
        let mut node = self.lock_node();
        let result = f(&mut node);
        for event in node.drain_events() {
            tracing::info!("{event}");
        }
        result
    }

    fn lock_node(&self) -> MutexGuard<'_, Node> {
        // 另一個 task 拿著鎖時 panic，鎖會被標成 poisoned；節點的狀態每一步都是完整的，繼續用沒問題。
        self.node
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_peers(&self) -> MutexGuard<'_, BTreeMap<u64, Peer>> {
        self.peers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 連線中的節點位址。
    pub fn peer_addresses(&self) -> Vec<String> {
        self.lock_peers()
            .values()
            .map(|p| p.address.clone())
            .collect()
    }

    fn add_peer(&self, address: String, sender: UnboundedSender<String>) -> u64 {
        let id = self.next_peer_id.fetch_add(1, Ordering::Relaxed);
        tracing::info!("節點 {address} 連上線");
        self.lock_peers().insert(id, Peer { address, sender });
        id
    }

    fn remove_peer(&self, id: u64) {
        if let Some(peer) = self.lock_peers().remove(&id) {
            tracing::info!("節點 {} 斷線", peer.address);
        }
    }

    /// 把 `Node` 要送的訊息送出去。`from` 是觸發這件事的節點（回覆給它）；從 HTTP API 來的時候是 `None`。
    pub fn dispatch(&self, from: Option<u64>, outbound: Vec<Outbound>) {
        let peers = self.lock_peers();
        for out in outbound {
            let (targets, message): (Vec<&Peer>, Message) = match out {
                Outbound::Reply(message) => (
                    from.and_then(|id| peers.get(&id)).into_iter().collect(),
                    message,
                ),
                Outbound::Broadcast(message) => (peers.values().collect(), message),
            };
            let text = serde_json::to_string(&message).expect("messages serialize to JSON");
            for peer in targets {
                // 送不出去代表那條連線正在關閉，等它自己從清單移除就好。
                let _ = peer.sender.send(text.clone());
            }
        }
    }
}

/// 啟動節點：產生或讀取金鑰、開始聽 HTTP 和 websocket、連到 `PEERS`。
pub async fn run(config: Config) -> Result<()> {
    let chain_key = keys::load_or_create_chain_key(&config.chain_key_file)?;
    let state = AppState::new(config.params.clone(), chain_key);
    let listener = TcpListener::bind(config.listen).await?;
    tracing::info!(
        "節點在 http://{} 上線（websocket 也是同一個 port）",
        listener.local_addr()?
    );
    for peer in config.peers.clone() {
        tokio::spawn(peers::connect_with_retry(state.clone(), peer));
    }
    serve(listener, state).await?;
    Ok(())
}

/// 在已經開好的 listener 上提供 HTTP API 和 websocket（測試會用 port 0 開一個隨機 port）。
pub async fn serve(listener: TcpListener, state: Arc<AppState>) -> std::io::Result<()> {
    let app = api::router(state);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}
