//! 第 7、8 章的練習場：在記憶體裡模擬一個 P2P 網路。
//!
//! 每個節點就是一個 [`Node`]；節點之間的「網路線」可以接上或拔掉，訊息會延遲一段時間才送到。
//! 時間是假的（模擬時鐘），所以測試每次跑的結果都一樣。網頁的「網路與分岔」頁面用的也是這份程式。

use std::collections::BTreeSet;

use serde::Serialize;

use crate::block::GENESIS_TIMESTAMP;
use crate::node::{Node, Outbound};
use crate::p2p::Message;
use crate::params::ChainParams;
use crate::transaction::{Transaction, TxBlock, TxError};
use crate::wallet::Wallet;

/// 一則還在路上的訊息。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InFlight {
    /// 什麼時候送到（模擬時鐘，毫秒）。
    pub deliver_at: u64,
    pub from: usize,
    pub to: usize,
    #[serde(skip)]
    pub message: Message,
    /// 給畫面顯示的簡短說明。
    pub summary: String,
    /// 送出的順序，時間一樣時先送先到。
    seq: u64,
}

/// 網路上發生的事。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub time: u64,
    pub node: usize,
    pub text: String,
}

/// 模擬的網路。
pub struct Network {
    nodes: Vec<Node>,
    links: BTreeSet<(usize, usize)>,
    in_flight: Vec<InFlight>,
    now: u64,
    latency_ms: u64,
    seq: u64,
    delivered: u64,
    log: Vec<LogEntry>,
}

impl Network {
    /// 建立 `count` 個節點，還沒接線。`start_time` 是模擬時鐘的起點（毫秒），要比創世區塊晚。
    pub fn new(count: usize, params: ChainParams, latency_ms: u64, start_time: u64) -> Self {
        let nodes = (0..count)
            .map(|_| Node::new(params.clone(), Wallet::generate(), None))
            .collect();
        Self::from_nodes(nodes, latency_ms, start_time)
    }

    /// 用自己建好的節點組成網路，例如共用同一把 FileChain 鏈金鑰的節點。
    pub fn from_nodes(nodes: Vec<Node>, latency_ms: u64, start_time: u64) -> Self {
        Network {
            nodes,
            links: BTreeSet::new(),
            in_flight: Vec::new(),
            now: start_time.max(GENESIS_TIMESTAMP),
            latency_ms,
            seq: 0,
            delivered: 0,
            log: Vec::new(),
        }
    }

    /// 到目前為止總共送達幾則訊息。
    pub fn delivered(&self) -> u64 {
        self.delivered
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn node(&self, index: usize) -> &Node {
        &self.nodes[index]
    }

    pub fn now(&self) -> u64 {
        self.now
    }

    pub fn in_flight(&self) -> &[InFlight] {
        &self.in_flight
    }

    pub fn log(&self) -> &[LogEntry] {
        &self.log
    }

    /// 目前接著的線，`(a, b)` 且 `a < b`。
    pub fn links(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.links.iter().copied()
    }

    pub fn is_connected(&self, a: usize, b: usize) -> bool {
        self.links.contains(&Self::key(a, b))
    }

    fn key(a: usize, b: usize) -> (usize, usize) {
        (a.min(b), a.max(b))
    }

    /// 接上兩個節點。跟真的節點一樣，接上之後雙方都先問對方最新的區塊。
    pub fn connect(&mut self, a: usize, b: usize) {
        if a == b || !self.links.insert(Self::key(a, b)) {
            return;
        }
        self.note(a, format!("和節點 {b} 接上線"));
        self.send(a, b, Node::greeting());
        self.send(b, a, Node::greeting());
    }

    /// 拔掉兩個節點之間的線。還在路上的訊息也會遺失。
    pub fn disconnect(&mut self, a: usize, b: usize) {
        if self.links.remove(&Self::key(a, b)) {
            self.in_flight
                .retain(|m| Self::key(m.from, m.to) != Self::key(a, b));
            self.note(a, format!("和節點 {b} 斷線"));
        }
    }

    /// 把所有節點兩兩接上。
    pub fn connect_all(&mut self) {
        for a in 0..self.nodes.len() {
            for b in a + 1..self.nodes.len() {
                self.connect(a, b);
            }
        }
    }

    /// 送一筆交易到某個節點（就像使用者呼叫那個節點的 API）。
    pub fn submit(&mut self, node: usize, tx: Transaction) -> Result<(), TxError> {
        let outbound = self.nodes[node].submit(tx)?;
        self.collect_events(node);
        self.dispatch(node, None, outbound);
        Ok(())
    }

    /// 讓某個節點挖一個區塊。
    pub fn mine(&mut self, node: usize) -> TxBlock {
        let (block, outbound) = self.nodes[node].mine(self.now);
        self.collect_events(node);
        self.dispatch(node, None, outbound);
        block
    }

    /// 讓時間前進 `ms` 毫秒，送達這段時間內到的訊息（包括途中新產生的訊息）。
    pub fn advance(&mut self, ms: u64) {
        let until = self.now.saturating_add(ms);
        while let Some(next) = self.next_delivery(until) {
            let message = self.in_flight.remove(next);
            self.now = self.now.max(message.deliver_at);
            self.deliver(message);
        }
        self.now = until;
    }

    /// 一直送訊息直到沒有訊息在路上。`max_messages` 防止寫錯的規則讓訊息無限繞圈。
    pub fn run_until_idle(&mut self, max_messages: usize) {
        for _ in 0..max_messages {
            let Some(next) = self.next_delivery(u64::MAX) else {
                return;
            };
            let message = self.in_flight.remove(next);
            self.now = self.now.max(message.deliver_at);
            self.deliver(message);
        }
    }

    /// 最早要送達、而且在 `until` 之前的訊息。
    fn next_delivery(&self, until: u64) -> Option<usize> {
        self.in_flight
            .iter()
            .enumerate()
            .filter(|(_, m)| m.deliver_at <= until)
            .min_by_key(|(_, m)| (m.deliver_at, m.seq))
            .map(|(i, _)| i)
    }

    fn deliver(&mut self, message: InFlight) {
        self.delivered += 1;
        let InFlight {
            from, to, message, ..
        } = message;
        let outbound = self.nodes[to].handle(message);
        self.collect_events(to);
        self.dispatch(to, Some(from), outbound);
    }

    fn dispatch(&mut self, from: usize, reply_to: Option<usize>, outbound: Vec<Outbound>) {
        for out in outbound {
            match out {
                Outbound::Reply(message) => {
                    if let Some(to) = reply_to {
                        self.send(from, to, message);
                    }
                }
                Outbound::Broadcast(message) => {
                    let neighbours: Vec<usize> = self
                        .links
                        .iter()
                        .filter_map(|&(a, b)| match (a == from, b == from) {
                            (true, _) => Some(b),
                            (_, true) => Some(a),
                            _ => None,
                        })
                        .collect();
                    for to in neighbours {
                        self.send(from, to, message.clone());
                    }
                }
            }
        }
    }

    fn send(&mut self, from: usize, to: usize, message: Message) {
        if !self.is_connected(from, to) {
            return;
        }
        self.seq += 1;
        self.in_flight.push(InFlight {
            deliver_at: self.now + self.latency_ms,
            from,
            to,
            summary: message.summary(),
            message,
            seq: self.seq,
        });
    }

    fn collect_events(&mut self, node: usize) {
        for event in self.nodes[node].drain_events() {
            self.note(node, event.to_string());
        }
    }

    fn note(&mut self, node: usize, text: String) {
        self.log.push(LogEntry {
            time: self.now,
            node,
            text,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: u64 = 1_700_000_000_000;

    fn network(count: usize) -> Network {
        let mut net = Network::new(count, ChainParams::fixed_difficulty(4), 100, START);
        net.connect_all();
        net.run_until_idle(1_000);
        net
    }

    fn tips(net: &Network) -> Vec<String> {
        net.nodes()
            .iter()
            .map(|n| n.chain().latest().hash.clone())
            .collect()
    }

    #[test]
    fn a_mined_block_reaches_everyone() {
        let mut net = network(3);
        let block = net.mine(0);
        net.run_until_idle(1_000);
        assert!(tips(&net).iter().all(|t| *t == block.hash));
    }

    #[test]
    fn messages_take_time() {
        let mut net = network(2);
        let block = net.mine(0);
        net.advance(50);
        assert_ne!(net.node(1).chain().latest().hash, block.hash);
        net.advance(50);
        assert_eq!(net.node(1).chain().latest().hash, block.hash);
    }

    #[test]
    fn a_line_topology_relays_transactions() {
        // 0 — 1 — 2：節點 0 和 2 沒有直接接上，交易靠節點 1 轉傳。
        let mut net = Network::new(3, ChainParams::fixed_difficulty(4), 100, START);
        net.connect(0, 1);
        net.connect(1, 2);
        net.run_until_idle(1_000);
        net.mine(0);
        net.run_until_idle(1_000);

        // 節點 0 的礦工把剛拿到的出塊獎勵轉 10 元給節點 2 的礦工。
        let to = net.node(2).address();
        let tx = Transaction::transfer(net.node(0).wallet(), &to, 10, START);
        net.submit(0, tx).unwrap();
        net.run_until_idle(1_000);
        assert_eq!(net.node(2).mempool().len(), 1);

        net.mine(2);
        net.run_until_idle(1_000);
        assert_eq!(net.node(0).ledger().balance(&to), 50 + 10);

        // 出塊獎勵不能從 API 送進來。
        let fake = Transaction::coinbase(&to, 1_000, 99);
        assert_eq!(net.submit(0, fake), Err(TxError::UnexpectedCoinbase));
    }

    #[test]
    fn disconnecting_drops_messages_in_flight() {
        let mut net = network(2);
        net.mine(0);
        net.disconnect(0, 1);
        net.run_until_idle(1_000);
        assert_eq!(net.node(1).chain().len(), 1);
    }
}
