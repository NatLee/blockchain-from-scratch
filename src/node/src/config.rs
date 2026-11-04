//! 節點的設定：全部從環境變數讀，名稱沿用原本專案的 `PORT` 和 `PEERS`。

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use anyhow::{Context, Result};
use blockchain_from_scratch::params::ChainParams;

/// 沒設 `PORT` 時用的 port，跟原本的節點一樣。
pub const DEFAULT_PORT: u16 = 3001;
/// 沒設 `CHAIN_KEY_FILE` 時，鏈的金鑰存在這裡（`.gitignore` 已經擋住 `data/`）。
pub const DEFAULT_CHAIN_KEY_FILE: &str = "data/chain-key.hex";
/// HTTP 請求的大小上限：20 MB，跟原本節點的 `REQUEST_MAX_SIZE` 一樣（檔案上傳要用）。
pub const MAX_REQUEST_BYTES: usize = 20 * 1024 * 1024;
/// 節點之間一則 websocket 訊息的上限。整條鏈裡可能有很多檔案，所以給得比較寬。
pub const MAX_P2P_MESSAGE_BYTES: usize = 128 * 1024 * 1024;
/// 啟動時連不上 `PEERS` 裡的節點，最多重試幾次、每次間隔幾秒（docker compose 裡節點會先後啟動）。
pub const PEER_CONNECT_ATTEMPTS: u32 = 15;
pub const PEER_RETRY_SECONDS: u64 = 1;

/// 節點啟動需要的設定。
#[derive(Clone, Debug)]
pub struct Config {
    /// HTTP API 和 websocket 共用的位址。
    pub listen: SocketAddr,
    /// 啟動時要連的節點，例如 `ws://node1:3001`。
    pub peers: Vec<String>,
    /// 鏈的加密金鑰檔。不存在就產生一把新的。
    pub chain_key_file: PathBuf,
    /// 鏈的規則。同一個網路的節點要一樣。
    pub params: ChainParams,
}

impl Config {
    /// 從環境變數讀設定：
    ///
    /// - `PORT`：預設 3001。
    /// - `BIND`：預設 `0.0.0.0`（所有網路介面）。
    /// - `PEERS`：用逗號分隔的 websocket 網址，例如 `ws://localhost:3001,ws://localhost:3002`。
    /// - `CHAIN_KEY_FILE`：預設 `data/chain-key.hex`。
    /// - `INITIAL_DIFFICULTY`：第 1 個區塊的難度，預設見 `ChainParams::default()`。
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// 跟 `from_env` 一樣，但從指定的函式讀值（測試用）。
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let port = match get("PORT") {
            Some(value) => value
                .parse()
                .with_context(|| format!("PORT={value} 不是 port 號碼"))?,
            None => DEFAULT_PORT,
        };
        let bind: IpAddr = match get("BIND") {
            Some(value) => value
                .parse()
                .with_context(|| format!("BIND={value} 不是 IP 位址"))?,
            None => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        };
        let peers = get("PEERS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|peer| !peer.is_empty())
            .map(str::to_string)
            .collect();
        let chain_key_file = get("CHAIN_KEY_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CHAIN_KEY_FILE));
        let mut params = ChainParams::default();
        if let Some(value) = get("INITIAL_DIFFICULTY") {
            params.initial_difficulty = value
                .parse()
                .with_context(|| format!("INITIAL_DIFFICULTY={value} 不是整數"))?;
            params.min_difficulty = params.min_difficulty.min(params.initial_difficulty);
        }
        Ok(Config {
            listen: SocketAddr::new(bind, port),
            peers,
            chain_key_file,
            params,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(vars: &[(&str, &str)]) -> Result<Config> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_lookup(|name| vars.get(name).cloned())
    }

    #[test]
    fn defaults_match_the_original_node() {
        let c = config(&[]).unwrap();
        assert_eq!(c.listen.port(), 3001);
        assert!(c.peers.is_empty());
        assert_eq!(c.chain_key_file, PathBuf::from(DEFAULT_CHAIN_KEY_FILE));
    }

    #[test]
    fn reads_port_peers_and_difficulty() {
        let c = config(&[
            ("PORT", "3002"),
            ("PEERS", "ws://localhost:3001, ws://localhost:3003,"),
            ("INITIAL_DIFFICULTY", "4"),
        ])
        .unwrap();
        assert_eq!(c.listen.port(), 3002);
        assert_eq!(c.peers, vec!["ws://localhost:3001", "ws://localhost:3003"]);
        assert_eq!(c.params.initial_difficulty, 4);
        assert_eq!(c.params.min_difficulty, 4);
    }

    #[test]
    fn rejects_bad_values() {
        assert!(config(&[("PORT", "abc")]).is_err());
        assert!(config(&[("INITIAL_DIFFICULTY", "-1")]).is_err());
    }
}
