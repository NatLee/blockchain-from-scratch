//! 節點程式的進入點。設定都從環境變數讀，見 `chain_node::Config::from_env`。
//!
//! ```sh
//! PORT=3001 cargo run -p chain-node
//! PORT=3002 PEERS=ws://localhost:3001 cargo run -p chain-node
//! ```

use tracing_subscriber::EnvFilter;

// `#[tokio::main]` 把 `async fn main` 包成一般的 main：建立 tokio 的執行環境，再跑這個 async 函式。
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let config = chain_node::Config::from_env()?;
    chain_node::run(config).await
}
