//! 把 core 編成 WebAssembly，讓瀏覽器裡的互動網頁直接跑教材的 Rust 程式。
//!
//! 這一層只做「格式轉換」：JavaScript 傳 JSON 字串進來，轉成 core 的型別，呼叫 core，
//! 再把結果轉回 JSON 字串。區塊鏈的邏輯一行都不在這裡。
//!
//! 錯誤用 `JsError` 回傳，在 JavaScript 那邊會變成丟出的 `Error`。

use std::collections::BTreeMap;
use std::fmt::Display;

use blockchain_from_scratch::block::Block;
use blockchain_from_scratch::chain::{self, BlockStatus};
use blockchain_from_scratch::filechain::{self, AddData, CerLevel, ChainKey, FileHit, Upload};
use blockchain_from_scratch::hash;
use blockchain_from_scratch::node::{Node, PublicKeyResponse, StatusResponse};
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::pow;
use blockchain_from_scratch::sim::Network;
use blockchain_from_scratch::transaction::{Transaction, TxBlock};
use blockchain_from_scratch::wallet::Wallet;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

type TextBlock = Block<String>;

fn js_error(error: impl Display) -> JsError {
    JsError::new(&error.to_string())
}

fn to_json<T: Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).expect("core types serialize to JSON")
}

fn from_json<T: DeserializeOwned>(text: &str) -> Result<T, JsError> {
    serde_json::from_str(text).map_err(|e| js_error(format!("JSON 格式不對：{e}")))
}

/// JavaScript 的數字是 f64；時間和金額都是非負整數，在這裡轉成 u64。
fn to_u64(value: f64) -> u64 {
    if value.is_finite() && value > 0.0 {
        value as u64
    } else {
        0
    }
}

fn decode_hash(hash_hex: &str) -> Result<[u8; 32], JsError> {
    hash::decode_hash_hex(hash_hex).ok_or_else(|| js_error("雜湊要是 64 個十六進位字元"))
}

// ───────────── 第 1 章：雜湊 ─────────────

#[wasm_bindgen(js_name = sha256Hex)]
pub fn sha256_hex(text: &str) -> String {
    hash::sha256_hex(text.as_bytes())
}

#[wasm_bindgen(js_name = sha256BytesHex)]
pub fn sha256_bytes_hex(bytes: &[u8]) -> String {
    hash::sha256_hex(bytes)
}

#[wasm_bindgen(js_name = bitDifference)]
pub fn bit_difference(a_hex: &str, b_hex: &str) -> Result<u32, JsError> {
    Ok(hash::bit_difference(
        &decode_hash(a_hex)?,
        &decode_hash(b_hex)?,
    ))
}

#[wasm_bindgen(js_name = leadingZeroBits)]
pub fn leading_zero_bits(hash_hex: &str) -> Result<u32, JsError> {
    Ok(hash::leading_zero_bits(&decode_hash(hash_hex)?))
}

// ───────────── 第 2–4 章：放文字的區塊鏈 ─────────────

/// 預設的網路參數（JSON）。
#[wasm_bindgen(js_name = defaultParams)]
pub fn default_params() -> String {
    to_json(&ChainParams::default())
}

/// 創世區塊（內容是空字串）。
#[wasm_bindgen(js_name = genesisBlock)]
pub fn genesis_block() -> String {
    to_json(&TextBlock::genesis(String::new()))
}

/// 接在 `blocks` 後面的新區塊（還沒挖礦）。
#[wasm_bindgen(js_name = nextBlock)]
pub fn next_block(
    blocks_json: &str,
    params_json: &str,
    timestamp: f64,
    data: &str,
) -> Result<String, JsError> {
    let blocks: Vec<TextBlock> = from_json(blocks_json)?;
    let params: ChainParams = from_json(params_json)?;
    let latest = blocks.last().ok_or_else(|| js_error("至少要有創世區塊"))?;
    let block = TextBlock::new(
        latest.header.index + 1,
        latest.hash.clone(),
        to_u64(timestamp).max(latest.header.timestamp),
        pow::next_difficulty(&blocks, &params),
        data.to_string(),
    );
    Ok(to_json(&block))
}

/// 重新算 `dataHash` 和 `hash`（`nonce` 不變）。網頁上改了內容或 previousHash 之後呼叫。
#[wasm_bindgen(js_name = rehashBlock)]
pub fn rehash_block(block_json: &str) -> Result<String, JsError> {
    let mut block: TextBlock = from_json(block_json)?;
    block.rehash();
    Ok(to_json(&block))
}

/// 標頭要拿去雜湊的字串。
#[wasm_bindgen(js_name = hashInput)]
pub fn hash_input(block_json: &str) -> Result<String, JsError> {
    let block: TextBlock = from_json(block_json)?;
    Ok(block.header.hash_input())
}

/// 從區塊目前的 nonce 開始，最多試 `max_attempts` 次。
/// 回傳 `{ found, block, attempts }`；沒找到時 `block.nonce` 是下一個要試的數字，下次接著呼叫。
#[wasm_bindgen(js_name = mineSteps)]
pub fn mine_steps(block_json: &str, max_attempts: u32) -> Result<String, JsError> {
    let mut block: TextBlock = from_json(block_json)?;
    let start = block.header.nonce;
    let (found, attempts) = match pow::try_mine(&mut block.header, u64::from(max_attempts)) {
        Some(hash) => {
            block.hash = hash;
            (true, block.header.nonce - start + 1)
        }
        None => (false, block.header.nonce - start),
    };
    Ok(to_json(
        &json!({ "found": found, "block": block, "attempts": attempts }),
    ))
}

/// 檢查整條鏈，回傳每個區塊的狀態：`{ status: "valid" | "invalid" | "buildsOnInvalid", reason? }`。
#[wasm_bindgen(js_name = checkTextChain)]
pub fn check_text_chain(blocks_json: &str, params_json: &str) -> Result<String, JsError> {
    let blocks: Vec<TextBlock> = from_json(blocks_json)?;
    let params: ChainParams = from_json(params_json)?;
    if blocks.is_empty() {
        return Ok("[]".into());
    }
    let statuses: Vec<Value> = chain::check_blocks(&blocks, &params)
        .into_iter()
        .map(|status| match status {
            BlockStatus::Valid => json!({ "status": "valid" }),
            BlockStatus::Invalid(error) => {
                json!({ "status": "invalid", "reason": error.to_string() })
            }
            BlockStatus::BuildsOnInvalid => json!({ "status": "buildsOnInvalid" }),
        })
        .collect();
    Ok(to_json(&statuses))
}

#[wasm_bindgen(js_name = nextDifficulty)]
pub fn next_difficulty(blocks_json: &str, params_json: &str) -> Result<u32, JsError> {
    let blocks: Vec<TextBlock> = from_json(blocks_json)?;
    let params: ChainParams = from_json(params_json)?;
    Ok(pow::next_difficulty(&blocks, &params))
}

/// 難度 `difficulty` 平均要試幾次（字串，因為可能超過 JavaScript 能精確表示的整數）。
#[wasm_bindgen(js_name = blockWork)]
pub fn block_work(difficulty: u32) -> String {
    pow::block_work(difficulty).to_string()
}

// ───────────── 第 5 章：金鑰、簽章和交易 ─────────────

/// 新錢包：`{ secret, address }`。私鑰只存在這個分頁的記憶體裡，重新整理就不見了。
#[wasm_bindgen(js_name = generateWallet)]
pub fn generate_wallet() -> String {
    let wallet = Wallet::generate();
    to_json(&json!({ "secret": wallet.secret_hex(), "address": wallet.address() }))
}

#[wasm_bindgen(js_name = signTransfer)]
pub fn sign_transfer(
    secret_hex: &str,
    to: &str,
    amount: f64,
    timestamp: f64,
) -> Result<String, JsError> {
    let wallet = Wallet::from_secret_hex(secret_hex).map_err(js_error)?;
    Ok(to_json(&Transaction::transfer(
        &wallet,
        to,
        to_u64(amount),
        to_u64(timestamp),
    )))
}

/// 驗證交易的簽章：`{ ok, error?, id }`。
#[wasm_bindgen(js_name = verifyTransaction)]
pub fn verify_transaction(tx_json: &str) -> Result<String, JsError> {
    let tx: Transaction = from_json(tx_json)?;
    let result = match tx.verify_signature() {
        Ok(()) => json!({ "ok": true, "id": tx.id() }),
        Err(e) => json!({ "ok": false, "error": e.to_string(), "id": tx.id() }),
    };
    Ok(to_json(&result))
}

// ───────────── 第 6–8 章：模擬網路 ─────────────

/// 記憶體裡的 P2P 網路（core 的 `sim::Network`）。
#[wasm_bindgen]
pub struct SimNetwork {
    inner: Network,
}

#[wasm_bindgen]
impl SimNetwork {
    /// `count` 個節點，訊息延遲 `latency_ms` 毫秒，模擬時鐘從 `start_time` 開始。
    #[wasm_bindgen(constructor)]
    pub fn new(
        count: usize,
        params_json: &str,
        latency_ms: f64,
        start_time: f64,
    ) -> Result<SimNetwork, JsError> {
        let params: ChainParams = from_json(params_json)?;
        Ok(SimNetwork {
            inner: Network::new(count, params, to_u64(latency_ms), to_u64(start_time)),
        })
    }

    pub fn connect(&mut self, a: usize, b: usize) {
        self.inner.connect(a, b);
    }

    pub fn disconnect(&mut self, a: usize, b: usize) {
        self.inner.disconnect(a, b);
    }

    #[wasm_bindgen(js_name = connectAll)]
    pub fn connect_all(&mut self) {
        self.inner.connect_all();
    }

    /// 把一筆簽好的交易送到某個節點。
    pub fn submit(&mut self, node: usize, tx_json: &str) -> Result<(), JsError> {
        let tx: Transaction = from_json(tx_json)?;
        self.inner.submit(node, tx).map_err(js_error)
    }

    /// 讓節點 `payer` 的礦工轉帳給 `to`，交易送到節點 `via`。
    /// `timestamp` 寫進交易內容：同一個人同時轉兩筆一樣的金額時，靠它讓兩筆交易不一樣。
    pub fn transfer(
        &mut self,
        via: usize,
        payer: usize,
        to: &str,
        amount: f64,
        timestamp: f64,
    ) -> Result<String, JsError> {
        let tx = Transaction::transfer(
            self.inner.node(payer).wallet(),
            to,
            to_u64(amount),
            to_u64(timestamp),
        );
        let id = tx.id();
        self.inner.submit(via, tx).map_err(js_error)?;
        Ok(id)
    }

    /// 讓節點挖一個區塊，回傳區塊（JSON）。
    pub fn mine(&mut self, node: usize) -> String {
        to_json(&self.inner.mine(node))
    }

    pub fn advance(&mut self, ms: f64) {
        self.inner.advance(to_u64(ms));
    }

    #[wasm_bindgen(js_name = runUntilIdle)]
    pub fn run_until_idle(&mut self) {
        self.inner.run_until_idle(10_000);
    }

    /// 某個節點的完整區塊（JSON）。
    #[wasm_bindgen(js_name = nodeBlocks)]
    pub fn node_blocks(&self, node: usize) -> String {
        to_json(self.inner.node(node).blocks())
    }

    /// 給畫面用的整體狀態（JSON）。
    pub fn snapshot(&self) -> String {
        let net = &self.inner;
        let nodes: Vec<Value> = net
            .nodes()
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let chain = node.chain();
                let blocks: Vec<Value> = chain.blocks().iter().map(block_summary).collect();
                let mempool: Vec<Value> = node
                    .mempool()
                    .transactions()
                    .iter()
                    .map(tx_summary)
                    .collect();
                json!({
                    "index": index,
                    "address": node.address(),
                    "height": chain.latest().header.index,
                    "tipHash": chain.latest().hash,
                    "totalWork": chain.total_work().to_string(),
                    "nextDifficulty": chain.next_difficulty(),
                    "blocks": blocks,
                    "mempool": mempool,
                    "balances": node.ledger().balances(),
                })
            })
            .collect();
        let links: Vec<[usize; 2]> = net.links().map(|(a, b)| [a, b]).collect();
        to_json(&json!({
            "now": net.now(),
            "delivered": net.delivered(),
            "links": links,
            "inFlight": net.in_flight(),
            "log": net.log(),
            "nodes": nodes,
        }))
    }
}

fn block_summary(block: &TxBlock) -> Value {
    let miner = block.data.first().and_then(|tx| match tx {
        Transaction::Coinbase { to, .. } => Some(to.clone()),
        _ => None,
    });
    json!({
        "index": block.header.index,
        "hash": block.hash,
        "previousHash": block.header.previous_hash,
        "timestamp": block.header.timestamp,
        "difficulty": block.header.difficulty,
        "miner": miner,
        "transactions": block.data.iter().map(tx_summary).collect::<Vec<_>>(),
    })
}

fn tx_summary(tx: &Transaction) -> Value {
    match tx {
        Transaction::Coinbase { to, amount, .. } => {
            json!({ "id": tx.id(), "type": "coinbase", "to": to, "amount": amount })
        }
        Transaction::Transfer(signed) => json!({
            "id": tx.id(),
            "type": "transfer",
            "from": signed.public_key,
            "to": signed.body.to,
            "amount": signed.body.amount,
        }),
        Transaction::File(signed) => json!({
            "id": tx.id(),
            "type": "file",
            "from": signed.public_key,
            "certificateOwner": signed.body.certificate_owner,
            "cerLevel": signed.body.cer_level,
            "unitName": signed.body.unit_name,
        }),
    }
}

// ───────────── 第 9 章：FileChain ─────────────

/// 跑在瀏覽器裡的一個節點，方法跟真的節點的 HTTP API 一一對應，回傳同樣格式的 JSON。
#[wasm_bindgen]
pub struct BrowserNode {
    inner: Node,
}

#[wasm_bindgen]
impl BrowserNode {
    /// 新節點：產生礦工錢包和鏈的加密金鑰（只存在這個分頁的記憶體裡）。
    #[wasm_bindgen(constructor)]
    pub fn new(params_json: &str) -> Result<BrowserNode, JsError> {
        let params: ChainParams = from_json(params_json)?;
        Ok(BrowserNode {
            inner: Node::new(params, Wallet::generate(), Some(ChainKey::generate())),
        })
    }

    /// `GET /getPublicKey`
    #[wasm_bindgen(js_name = getPublicKey)]
    pub fn get_public_key(&self) -> String {
        to_json(&PublicKeyResponse {
            public_key: self.inner.chain_public_key().unwrap_or_default(),
        })
    }

    /// `POST /addData`：成功回傳 pool，失敗丟出錯誤。
    #[wasm_bindgen(js_name = addData)]
    pub fn add_data(&mut self, data_json: &str) -> Result<String, JsError> {
        let data: AddData = from_json(data_json)?;
        self.inner.add_data(data).map_err(js_error)?;
        Ok(self.pool_list())
    }

    /// `POST /addTransaction`
    #[wasm_bindgen(js_name = addTransaction)]
    pub fn add_transaction(&mut self, tx_json: &str) -> Result<String, JsError> {
        let tx: Transaction = from_json(tx_json)?;
        self.inner.submit(tx).map_err(js_error)?;
        Ok(self.pool_list())
    }

    /// `GET /poolList`
    #[wasm_bindgen(js_name = poolList)]
    pub fn pool_list(&self) -> String {
        to_json(self.inner.mempool().transactions())
    }

    /// `POST /mineBlock`：回傳新區塊；pool 是空的時候回傳 `{status: false, message}`，跟真的節點一樣。
    #[wasm_bindgen(js_name = mineBlock)]
    pub fn mine_block(&mut self, timestamp: f64) -> String {
        match self.inner.mine_pool(to_u64(timestamp)) {
            Ok((block, _)) => to_json(&block),
            Err(e) => to_json(&StatusResponse {
                status: false,
                message: Some(e.to_string()),
            }),
        }
    }

    /// `GET /blocks`
    pub fn blocks(&self) -> String {
        to_json(self.inner.blocks())
    }

    /// `GET /queryBlockByName?queryName=…`
    #[wasm_bindgen(js_name = queryBlockByName)]
    pub fn query_block_by_name(&self, name: &str) -> Result<String, JsError> {
        let hits = self.inner.query_by_name(name).map_err(js_error)?;
        Ok(to_json(&hits))
    }

    /// `GET /balances`
    pub fn balances(&self) -> String {
        let balances: &BTreeMap<String, u64> = self.inner.ledger().balances();
        to_json(balances)
    }

    /// `GET /peers`：瀏覽器裡的節點沒有連到別人。
    pub fn peers(&self) -> String {
        "[]".into()
    }
}

/// 客戶端：壓縮、用鏈的公鑰加密、用單位的私鑰簽章，回傳 `POST /addData` 要送的 JSON。
#[wasm_bindgen(js_name = prepareUpload)]
pub fn prepare_upload(
    file: &[u8],
    certificate_owner: &str,
    cer_level: &str,
    unit_name: &str,
    unit_secret_hex: &str,
    chain_public_hex: &str,
    timestamp: f64,
) -> Result<String, JsError> {
    let level: CerLevel = from_json(&to_json(cer_level))?;
    let unit = Wallet::from_secret_hex(unit_secret_hex).map_err(js_error)?;
    let upload = Upload {
        file,
        certificate_owner,
        cer_level: level,
        unit_name,
        timestamp: to_u64(timestamp),
    };
    let data = filechain::prepare_upload(upload, &unit, chain_public_hex).map_err(js_error)?;
    Ok(to_json(&data))
}

/// 客戶端：把查到的檔案解壓縮並確認雜湊，回傳原始檔案的 bytes。
#[wasm_bindgen(js_name = readHit)]
pub fn read_hit(hit_json: &str) -> Result<Vec<u8>, JsError> {
    let hit: FileHit = from_json(hit_json)?;
    filechain::read_hit(&hit).map_err(js_error)
}

/// 地址（公鑰）是不是合法的格式。
#[wasm_bindgen(js_name = isAddress)]
pub fn is_address(address: &str) -> bool {
    blockchain_from_scratch::wallet::is_address(address)
}

/// 錢包私鑰對應的地址。
#[wasm_bindgen(js_name = addressOf)]
pub fn address_of(secret_hex: &str) -> Result<String, JsError> {
    Ok(Wallet::from_secret_hex(secret_hex)
        .map_err(js_error)?
        .address())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 這些測試在一般的電腦上跑（不用瀏覽器），確認 JSON 格式轉換沒問題。
    // `JsError` 只能在 wasm 裡建立，所以這裡只測成功的路徑。

    #[test]
    fn text_chain_round_trip() {
        let params = to_json(&ChainParams::fixed_difficulty(4));
        let genesis = genesis_block();
        let blocks = format!("[{genesis}]");
        let template = next_block(&blocks, &params, 1.7e12, "hello").unwrap();
        let mut mined: Value = serde_json::from_str(&mine_steps(&template, 1).unwrap()).unwrap();
        while mined["found"] != true {
            mined = serde_json::from_str(&mine_steps(&mined["block"].to_string(), 100).unwrap())
                .unwrap();
        }
        let chain = format!("[{genesis},{}]", mined["block"]);
        let statuses: Value =
            serde_json::from_str(&check_text_chain(&chain, &params).unwrap()).unwrap();
        assert_eq!(
            statuses,
            json!([{ "status": "valid" }, { "status": "valid" }])
        );
    }

    #[test]
    fn browser_node_matches_the_http_api_shapes() {
        let params = to_json(&ChainParams::fixed_difficulty(4));
        let mut node = BrowserNode::new(&params).unwrap();
        let key: Value = serde_json::from_str(&node.get_public_key()).unwrap();
        let wallet: Value = serde_json::from_str(&generate_wallet()).unwrap();
        let data = prepare_upload(
            b"file",
            "Miles",
            "Low",
            "AMERICA",
            wallet["secret"].as_str().unwrap(),
            key["public_key"].as_str().unwrap(),
            1.7e12,
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&node.add_data(&data).unwrap())
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let block: Value = serde_json::from_str(&node.mine_block(1.7e12)).unwrap();
        assert_eq!(block["index"], 1);
        let empty: Value = serde_json::from_str(&node.mine_block(1.7e12)).unwrap();
        assert_eq!(empty["status"], false);
        let hits: Value =
            serde_json::from_str(&node.query_block_by_name("Miles").unwrap()).unwrap();
        assert_eq!(read_hit(&hits[0].to_string()).unwrap(), b"file");
    }

    #[test]
    fn sim_network_snapshot() {
        let params = to_json(&ChainParams::fixed_difficulty(4));
        let mut net = SimNetwork::new(3, &params, 100.0, 1.7e12).unwrap();
        net.connect_all();
        net.mine(0);
        net.run_until_idle();
        let to = net.inner.node(1).address();
        net.transfer(0, 0, &to, 10.0, 1.7e12).unwrap();
        net.run_until_idle();
        let snapshot: Value = serde_json::from_str(&net.snapshot()).unwrap();
        assert_eq!(snapshot["nodes"][2]["height"], 1);
        assert_eq!(snapshot["nodes"][2]["mempool"][0]["amount"], 10);
        assert_eq!(snapshot["links"].as_array().unwrap().len(), 3);
    }
}
