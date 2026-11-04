//! 節點之間的 websocket 連線（第 7 章）。
//!
//! 每條連線有兩個方向：
//!
//! - **收**：一直讀對方送來的訊息，交給 `Node::handle`，再把結果送出去。
//! - **送**：其他 task 要送訊息給這個節點時，丟進一個 channel；一個專門的 task 從 channel 拿出來寫到 websocket。
//!
//! 別人連進來（`GET /` 升級成 websocket，axum）和我們連出去（`tokio-tungstenite`）用的是不同的 crate，
//! 但轉成「一串文字訊息」之後，處理方式完全一樣，都交給 [`run_peer`]。

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use blockchain_from_scratch::node::Node;
use blockchain_from_scratch::p2p::Message;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite;

use crate::AppState;
use crate::config::{MAX_P2P_MESSAGE_BYTES, PEER_CONNECT_ATTEMPTS, PEER_RETRY_SECONDS};

/// 處理一條已經建立好的連線，直到斷線為止。
///
/// `incoming` 是對方送來的文字訊息，`outgoing` 是寫回去的地方。
/// `async fn` 不會馬上執行，而是回傳一個 future；被 `.await` 或 `tokio::spawn` 時才真的開始跑。
pub async fn run_peer<In, Out>(state: Arc<AppState>, address: String, incoming: In, outgoing: Out)
where
    In: Stream<Item = String> + Send,
    Out: Sink<String> + Send + 'static,
{
    let (sender, mut receiver) = mpsc::unbounded_channel::<String>();
    let id = state.add_peer(address, sender.clone());

    // 送的方向：另開一個 task，從 channel 拿訊息寫到 websocket。
    // `move` 把 `receiver` 和 `outgoing` 的所有權交給這個 task。
    let writer = tokio::spawn(async move {
        let mut outgoing = Box::pin(outgoing);
        while let Some(text) = receiver.recv().await {
            if outgoing.send(text).await.is_err() {
                break;
            }
        }
    });

    // 一連上就先問對方最新的區塊。
    let greeting = serde_json::to_string(&Node::greeting()).expect("messages serialize to JSON");
    let _ = sender.send(greeting);

    // 收的方向：在這個 task 裡一直讀。`pin!` 把 stream 固定在記憶體裡，`next().await` 才能用。
    let mut incoming = std::pin::pin!(incoming);
    while let Some(text) = incoming.next().await {
        match serde_json::from_str::<Message>(&text) {
            Ok(message) => {
                tracing::debug!("收到 {}", message.summary());
                let outbound = state.with_node(|node| node.handle(message));
                state.dispatch(Some(id), outbound);
            }
            Err(e) => tracing::warn!("看不懂的訊息：{e}"),
        }
    }

    state.remove_peer(id);
    writer.abort();
}

/// 連到另一個節點（`POST /addPeer`，或啟動時的 `PEERS`）。
pub async fn connect(state: Arc<AppState>, url: String) -> Result<()> {
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_P2P_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_P2P_MESSAGE_BYTES));
    let (socket, _) =
        tokio_tungstenite::connect_async_with_config(url.as_str(), Some(config), false)
            .await
            .with_context(|| format!("連不上 {url}"))?;
    let (sink, stream) = socket.split();
    let incoming = stream.filter_map(|frame| async move {
        match frame {
            Ok(tungstenite::Message::Text(text)) => Some(text.as_str().to_owned()),
            _ => None,
        }
    });
    let outgoing = sink.with(|text: String| async move {
        Ok::<_, tungstenite::Error>(tungstenite::Message::text(text))
    });
    // 連線本身在背景跑，這個函式連上就回傳。
    tokio::spawn(run_peer(state, url, incoming, outgoing));
    Ok(())
}

/// 啟動時連 `PEERS`：對方可能還沒開好，所以失敗就等一下再試。
pub async fn connect_with_retry(state: Arc<AppState>, url: String) {
    for attempt in 1..=PEER_CONNECT_ATTEMPTS {
        match connect(state.clone(), url.clone()).await {
            Ok(()) => return,
            Err(e) => {
                tracing::warn!("第 {attempt} 次連 {url} 失敗：{e:#}");
                tokio::time::sleep(Duration::from_secs(PEER_RETRY_SECONDS)).await;
            }
        }
    }
    tracing::error!("放棄連線 {url}");
}
