//! # blockchain-from-scratch
//!
//! 從零開始寫一條區塊鏈。每一章對應一個 module，每章都有可以跑的範例（`cargo run --example chNN-…`）
//! 和說明文件（`docs/chapters/`）。
//!
//! | 章 | 主題 | module | 範例 |
//! |---|---|---|---|
//! | 1 | 雜湊 | [`hash`] | `ch01-hash` |
//! | 2 | 區塊 | [`block`] | `ch02-block` |
//! | 3 | 鏈和竄改偵測 | [`chain`] | `ch03-chain` |
//! | 4 | 工作量證明和難度 | [`pow`]、[`params`] | `ch04-pow` |
//! | 5 | 金鑰、簽章和交易 | [`wallet`]、[`transaction`] | `ch05-signature` |
//! | 6 | mempool 和出塊 | [`ledger`]、[`mempool`]、[`miner`] | `ch06-mempool` |
//! | 7 | P2P 節點和廣播 | [`p2p`]、[`node`]、[`sim`] | `ch07-p2p` |
//! | 8 | 分岔和共識 | [`consensus`] | `ch08-fork` |
//! | 9 | FileChain | [`filechain`] | `ch09-filechain` |
//!
//! 這個 crate 不碰網路，也不讀系統時鐘：需要時間的函式都由呼叫的人把時間（毫秒）傳進來。
//! 所以同一份程式可以跑在節點程式（`src/node`）、瀏覽器（`src/wasm`）和測試裡。

pub mod block;
pub mod chain;
pub mod consensus;
pub mod filechain;
pub mod hash;
pub mod ledger;
pub mod mempool;
pub mod miner;
pub mod node;
pub mod p2p;
pub mod params;
pub mod pow;
pub mod sim;
pub mod transaction;
pub mod wallet;
