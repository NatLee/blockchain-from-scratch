//! 每一章「動手做」的解答。文件（`docs/chapters/`）裡的解答就是這裡的程式，
//! 每個解答都有測試，確認它真的做到題目要的事。
//!
//! 先自己寫寫看，卡住了再來看。執行：`cargo test -p blockchain-from-scratch --test exercises`

const START: u64 = 1_700_000_000_000;

/// 第 1 章：雜湊
mod ch01 {
    use blockchain_from_scratch::hash::{bit_difference, sha256, sha256_hex};

    /// 練習 1：找一個最小的 nonce，讓 `sha256("{message}{nonce}")` 的十六進位開頭有 `zeros` 個 0。
    pub fn find_nonce(message: &str, zeros: usize) -> (u64, String) {
        let prefix = "0".repeat(zeros);
        (0u64..)
            .map(|nonce| (nonce, sha256_hex(format!("{message}{nonce}").as_bytes())))
            .find(|(_, hash)| hash.starts_with(&prefix))
            .expect("an infinite range always finds one eventually")
    }

    /// 練習 2：連續的整數（"0" 和 "1"、"1" 和 "2"…）的雜湊，平均差幾個 bit？
    pub fn average_bit_difference(samples: u32) -> f64 {
        let total: u32 = (0..samples)
            .map(|i| {
                bit_difference(
                    &sha256(i.to_string().as_bytes()),
                    &sha256((i + 1).to_string().as_bytes()),
                )
            })
            .sum();
        f64::from(total) / f64::from(samples)
    }

    #[test]
    fn finds_a_hash_with_two_leading_zeros() {
        let (nonce, hash) = find_nonce("hello", 2);
        assert!(hash.starts_with("00"));
        assert_eq!(sha256_hex(format!("hello{nonce}").as_bytes()), hash);
    }

    #[test]
    fn about_half_of_the_bits_change() {
        let average = average_bit_difference(1_000);
        assert!((120.0..136.0).contains(&average), "average = {average}");
    }
}

/// 第 2 章：區塊
mod ch02 {
    use blockchain_from_scratch::block::{Block, data_hash};
    use blockchain_from_scratch::hash::bit_difference;

    /// 練習 1：同樣兩筆資料，交換順序之後 `data_hash` 一樣嗎？
    pub fn order_matters() -> bool {
        let ab = vec![
            "Alice pays Bob 10".to_string(),
            "Bob pays Carol 5".to_string(),
        ];
        let ba = vec![ab[1].clone(), ab[0].clone()];
        data_hash(&ab) != data_hash(&ba)
    }

    /// 練習 2：時間改 1 毫秒，區塊雜湊有幾個 bit 不一樣？
    pub fn one_millisecond_difference() -> u32 {
        let a = Block::new(1, "00".repeat(32), super::START, 0, "data".to_string());
        let mut b = a.clone();
        b.header.timestamp += 1;
        b.rehash();
        bit_difference(
            &hex::decode(&a.hash).unwrap(),
            &hex::decode(&b.hash).unwrap(),
        )
    }

    #[test]
    fn swapping_data_changes_the_hash() {
        assert!(order_matters());
    }

    #[test]
    fn a_millisecond_flips_many_bits() {
        let bits = one_millisecond_difference();
        assert!((60..=196).contains(&bits), "bits = {bits}");
    }
}

/// 第 3 章：鏈和竄改偵測
mod ch03 {
    use blockchain_from_scratch::block::Block;
    use blockchain_from_scratch::chain::{BlockStatus, Blockchain, check_blocks};
    use blockchain_from_scratch::params::ChainParams;

    /// 練習 1：找出第一個被改過（或接不上）的區塊。
    pub fn first_bad_block(blocks: &[Block<String>], params: &ChainParams) -> Option<usize> {
        check_blocks(blocks, params)
            .iter()
            .position(|status| *status != BlockStatus::Valid)
    }

    /// 練習 2：改掉第 `index` 個區塊的內容，再把它和後面的區塊全部重算雜湊。
    pub fn rewrite_history(
        blocks: &[Block<String>],
        index: usize,
        data: &str,
    ) -> Vec<Block<String>> {
        let mut forged = blocks.to_vec();
        forged[index].data = data.to_string();
        for i in index..forged.len() {
            forged[i].header.previous_hash = forged[i - 1].hash.clone();
            forged[i].rehash();
        }
        forged
    }

    fn chain() -> Blockchain<String> {
        let mut chain = Blockchain::new(ChainParams::fixed_difficulty(0));
        for (i, text) in ["a", "b", "c", "d"].iter().enumerate() {
            let block = chain.next_block(super::START + i as u64, text.to_string());
            chain.push(block).unwrap();
        }
        chain
    }

    #[test]
    fn finds_the_tampered_block() {
        let chain = chain();
        let mut blocks = chain.blocks().to_vec();
        assert_eq!(first_bad_block(&blocks, chain.params()), None);
        blocks[2].data = "B".into();
        assert_eq!(first_bad_block(&blocks, chain.params()), Some(2));
    }

    #[test]
    fn without_proof_of_work_rewriting_history_goes_unnoticed() {
        let chain = chain();
        let forged = rewrite_history(chain.blocks(), 1, "Alice pays Mallory 1000");
        assert_eq!(first_bad_block(&forged, chain.params()), None);
        assert_ne!(forged, chain.blocks());
    }
}

/// 第 4 章：工作量證明和難度
mod ch04 {
    use blockchain_from_scratch::block::Block;
    use blockchain_from_scratch::pow::{chain_work, mine};

    /// 練習 1：難度 `difficulty` 平均要試幾次？挖 `samples` 個區塊算平均。
    pub fn average_attempts(difficulty: u32, samples: u64) -> f64 {
        let total: u64 = (0..samples)
            .map(|i| {
                let block = Block::new(
                    1,
                    "00".repeat(32),
                    super::START + i,
                    difficulty,
                    i.to_string(),
                );
                mine(block).1
            })
            .sum();
        total as f64 / samples as f64
    }

    /// 練習 2：攻擊者想改第 `from` 個區塊，至少要重做多少工作量？（被改的和後面全部都要重挖）
    pub fn rewrite_cost<D>(blocks: &[Block<D>], from: usize) -> u128 {
        chain_work(&blocks[from..])
    }

    #[test]
    fn average_attempts_is_about_two_to_the_difficulty() {
        let average = average_attempts(6, 300);
        assert!((32.0..128.0).contains(&average), "average = {average}");
    }

    #[test]
    fn older_blocks_cost_more_to_rewrite() {
        let blocks: Vec<Block<String>> = (1..=5)
            .map(|i| Block::new(i, "00".repeat(32), super::START, 10, String::new()))
            .collect();
        assert_eq!(rewrite_cost(&blocks, 4), 1024);
        assert_eq!(rewrite_cost(&blocks, 0), 5 * 1024);
    }
}

/// 第 5 章：金鑰、簽章和交易
mod ch05 {
    use blockchain_from_scratch::ledger::Ledger;
    use blockchain_from_scratch::transaction::{Transaction, TxError};
    use blockchain_from_scratch::wallet::Wallet;

    /// 練習 1：Mallory 截到 Alice 給 Bob 的交易，把收款人改成自己。
    pub fn redirect(tx: &Transaction, to: &str) -> Transaction {
        let Transaction::Transfer(signed) = tx else {
            panic!("only transfers have a recipient");
        };
        let mut forged = signed.clone();
        forged.body.to = to.to_string();
        Transaction::Transfer(forged)
    }

    #[test]
    fn redirected_transfer_has_a_bad_signature() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let mallory = Wallet::generate();
        let tx = Transaction::transfer(&alice, &bob.address(), 10, super::START);
        assert_eq!(
            redirect(&tx, &mallory.address()).verify_signature(),
            Err(TxError::BadSignature)
        );
    }

    /// 練習 2：同一筆交易送兩次（重送攻擊），帳本會怎麼處理？
    #[test]
    fn replaying_a_transaction_is_rejected() {
        let alice = Wallet::generate();
        let bob = Wallet::generate();
        let mut ledger = Ledger::default();
        // 給 Alice 一些錢：一個只有出塊獎勵的區塊。
        let params = blockchain_from_scratch::params::ChainParams::fixed_difficulty(0);
        let mut chain = blockchain_from_scratch::chain::Blockchain::new(params.clone());
        let reward = vec![Transaction::coinbase(
            &alice.address(),
            params.block_reward,
            1,
        )];
        let block = chain.next_block(super::START, reward);
        chain.push(block).unwrap();
        ledger.apply_block(&chain.blocks()[1], &params).unwrap();

        let tx = Transaction::transfer(&alice, &bob.address(), 10, super::START);
        ledger.apply(&tx).unwrap();
        assert_eq!(ledger.apply(&tx), Err(TxError::Duplicate));
        // 時間不同就是另一筆交易，可以再轉一次。
        let again = Transaction::transfer(&alice, &bob.address(), 10, super::START + 1);
        assert_eq!(ledger.apply(&again), Ok(()));
        assert_eq!(ledger.balance(&bob.address()), 20);
    }
}

/// 第 6 章：mempool 和出塊
mod ch06 {
    use blockchain_from_scratch::chain::Blockchain;
    use blockchain_from_scratch::ledger::Ledger;
    use blockchain_from_scratch::mempool::Mempool;
    use blockchain_from_scratch::miner::mine_next;
    use blockchain_from_scratch::params::ChainParams;
    use blockchain_from_scratch::transaction::{Transaction, TxBlock, TxError};
    use blockchain_from_scratch::wallet::Wallet;

    /// 練習 1：每個區塊最多放 `max` 筆交易（不含出塊獎勵）。
    pub fn assemble_limited(
        chain: &Blockchain<Vec<Transaction>>,
        mempool: &Mempool,
        miner: &str,
        timestamp: u64,
        max: usize,
    ) -> TxBlock {
        let height = chain.latest().header.index + 1;
        let mut data = vec![Transaction::coinbase(
            miner,
            chain.params().block_reward,
            height,
        )];
        data.extend(mempool.transactions().iter().take(max).cloned());
        chain.next_block(timestamp, data)
    }

    fn funded() -> (Blockchain<Vec<Transaction>>, Ledger, Wallet) {
        let params = ChainParams::fixed_difficulty(0);
        let alice = Wallet::generate();
        let mut chain = Blockchain::new(params.clone());
        let (block, _) = mine_next(&chain, &Mempool::default(), &alice.address(), super::START);
        chain.push(block).unwrap();
        let ledger = Ledger::from_blocks(chain.blocks(), &params).unwrap();
        (chain, ledger, alice)
    }

    #[test]
    fn block_size_limit() {
        let (chain, ledger, alice) = funded();
        let mut pool = Mempool::default();
        for i in 0..5 {
            pool.add(
                Transaction::transfer(&alice, &Wallet::generate().address(), 1, i),
                &ledger,
            )
            .unwrap();
        }
        let block = assemble_limited(&chain, &pool, &alice.address(), super::START + 1, 3);
        assert_eq!(block.data.len(), 1 + 3);
    }

    /// 練習 2：Alice 有 50 元，同時送出兩筆 30 元的轉帳，mempool 收幾筆？
    #[test]
    fn only_one_of_two_overlapping_payments_fits() {
        let (_, ledger, alice) = funded();
        let mut pool = Mempool::default();
        let bob = Wallet::generate().address();
        let carol = Wallet::generate().address();
        assert!(
            pool.add(Transaction::transfer(&alice, &bob, 30, 1), &ledger)
                .is_ok()
        );
        assert_eq!(
            pool.add(Transaction::transfer(&alice, &carol, 30, 2), &ledger),
            Err(TxError::InsufficientFunds {
                balance: 20,
                amount: 30
            })
        );
        assert_eq!(pool.len(), 1);
    }
}

/// 第 7 章：P2P 節點和廣播
mod ch07 {
    use blockchain_from_scratch::params::ChainParams;
    use blockchain_from_scratch::sim::Network;

    /// 練習 1：節點 3 晚加入，接上線之後能不能自己補齊整條鏈？
    #[test]
    fn a_late_node_catches_up() {
        let mut net = Network::new(4, ChainParams::fixed_difficulty(4), 100, super::START);
        net.connect(0, 1);
        net.connect(1, 2);
        net.run_until_idle(1_000);
        for _ in 0..3 {
            net.mine(0);
            net.run_until_idle(1_000);
        }
        assert_eq!(net.node(3).chain().len(), 1);

        net.connect(3, 2);
        net.run_until_idle(1_000);
        assert_eq!(
            net.node(3).chain().latest().hash,
            net.node(0).chain().latest().hash
        );
    }

    /// 練習 2：3 個節點兩兩相連，挖一個區塊總共會送幾則訊息？
    pub fn messages_for_one_block() -> u64 {
        let mut net = Network::new(3, ChainParams::fixed_difficulty(4), 100, super::START);
        net.connect_all();
        net.run_until_idle(1_000);
        let before = net.delivered();
        net.mine(0);
        net.run_until_idle(1_000);
        net.delivered() - before
    }

    #[test]
    fn one_block_costs_six_messages() {
        // 節點 0 送給 1 和 2（2 則）；1 和 2 接上區塊後各自再轉傳給另外兩個節點（4 則）。
        // 收到已經有的區塊就不再轉傳，所以總共 6 則。
        assert_eq!(messages_for_one_block(), 6);
    }
}

/// 第 8 章：分岔和共識
mod ch08 {
    use blockchain_from_scratch::params::ChainParams;
    use blockchain_from_scratch::sim::Network;

    fn tips(net: &Network) -> Vec<String> {
        net.nodes()
            .iter()
            .map(|n| n.chain().latest().hash.clone())
            .collect()
    }

    /// 練習 1：攻擊者（節點 0）偷偷斷線，私下挖了 3 個區塊；其他人只挖了 1 個。重新接上後會怎樣？
    #[test]
    fn a_private_longer_chain_takes_over() {
        let mut net = Network::new(3, ChainParams::fixed_difficulty(4), 100, super::START);
        net.connect_all();
        net.run_until_idle(1_000);
        net.disconnect(0, 1);
        net.disconnect(0, 2);
        for _ in 0..3 {
            net.mine(0);
        }
        net.mine(1);
        net.run_until_idle(1_000);

        net.connect(0, 1);
        net.connect(0, 2);
        net.run_until_idle(1_000);
        let tips = tips(&net);
        assert!(tips.iter().all(|t| *t == net.node(0).chain().latest().hash));
        assert_eq!(net.node(1).chain().len(), 4);
    }

    /// 練習 2：兩邊各挖一個區塊（一樣長、一樣難度），接上後誰贏？
    #[test]
    fn a_tie_waits_for_the_next_block() {
        let mut net = Network::new(2, ChainParams::fixed_difficulty(4), 100, super::START);
        net.mine(0);
        net.mine(1);
        net.connect(0, 1);
        net.run_until_idle(1_000);
        let t = tips(&net);
        assert_ne!(t[0], t[1], "工作量一樣，各自留著先看到的那條");

        net.mine(1);
        net.run_until_idle(1_000);
        let t = tips(&net);
        assert_eq!(t[0], t[1], "下一個區塊打破平手");
    }
}

/// 第 9 章：FileChain
mod ch09 {
    use std::collections::HashMap;

    use blockchain_from_scratch::filechain::{CerLevel, ChainKey, FileHit, Upload, prepare_upload};
    use blockchain_from_scratch::node::Node;
    use blockchain_from_scratch::params::ChainParams;
    use blockchain_from_scratch::transaction::TxError;
    use blockchain_from_scratch::wallet::Wallet;

    /// 練習 1：只相信名單上的單位。`registry` 是「公鑰 → 單位名稱」，跟原本程式寫死的
    /// `loc_lookup` 一樣；不在名單上的單位不算，名稱以名單為準（不採用單位自己宣告的名稱）。
    pub fn trusted_units(
        hits: &[FileHit],
        registry: &HashMap<String, String>,
    ) -> Vec<(String, u32)> {
        let mut counts: Vec<(String, u32)> = Vec::new();
        for certification in hits.iter().flat_map(|hit| &hit.certifications) {
            let Some(name) = registry.get(&certification.public_key) else {
                continue;
            };
            match counts.iter_mut().find(|(n, _)| n == name) {
                Some((_, count)) => *count += certification.count,
                None => counts.push((name.clone(), certification.count)),
            }
        }
        counts
    }

    fn node_with(key: &ChainKey) -> Node {
        Node::new(
            ChainParams::fixed_difficulty(4),
            Wallet::generate(),
            Some(key.clone()),
        )
    }

    fn upload<'a>(file: &'a [u8], unit_name: &'a str) -> Upload<'a> {
        Upload {
            file,
            certificate_owner: "Miles",
            cer_level: CerLevel::Low,
            unit_name,
            timestamp: super::START,
        }
    }

    #[test]
    fn only_registered_units_count() {
        let key = ChainKey::generate();
        let mut node = node_with(&key);
        let america = Wallet::generate();
        let impostor = Wallet::generate();
        for (unit, name) in [(&america, "AMERICA"), (&impostor, "AMERICA")] {
            let data = prepare_upload(upload(b"file", name), unit, &key.public_hex()).unwrap();
            node.add_data(data).unwrap();
        }
        node.mine_pool(super::START).unwrap();

        let hits = node.query_by_name("Miles").unwrap();
        let registry = HashMap::from([(america.address(), "AMERICA".to_string())]);
        assert_eq!(
            trusted_units(&hits, &registry),
            vec![("AMERICA".to_string(), 1)]
        );
    }

    /// 練習 2：有人在轉傳途中改了密文（`base64Data`）。節點會收嗎？
    #[test]
    fn tampered_upload_is_rejected() {
        let key = ChainKey::generate();
        let mut node = node_with(&key);
        let unit = Wallet::generate();
        let mut data =
            prepare_upload(upload(b"file", "AMERICA"), &unit, &key.public_hex()).unwrap();
        let mut chars: Vec<char> = data.base64_data.chars().collect();
        chars[10] = if chars[10] == 'A' { 'B' } else { 'A' };
        data.base64_data = chars.into_iter().collect();
        assert_eq!(node.add_data(data).unwrap_err(), TxError::BadSignature);
    }
}
