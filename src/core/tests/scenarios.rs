//! 跨章節的情境測試：把好幾章的東西組起來，確認整體行為跟教材說的一樣。

use blockchain_from_scratch::chain::{BlockError, BlockStatus, Blockchain, check_blocks};
use blockchain_from_scratch::filechain::{CerLevel, ChainKey, Upload, prepare_upload, read_hit};
use blockchain_from_scratch::node::Node;
use blockchain_from_scratch::params::ChainParams;
use blockchain_from_scratch::pow::{self, mine};
use blockchain_from_scratch::sim::Network;
use blockchain_from_scratch::transaction::Transaction;
use blockchain_from_scratch::wallet::Wallet;

const START: u64 = 1_700_000_000_000;

/// 第 3、4 章：沒有工作量證明時，改歷史只要重算雜湊；有了工作量證明，重算雜湊不夠，還得重新挖礦。
#[test]
fn proof_of_work_makes_rewriting_history_expensive() {
    for difficulty in [0, 8] {
        let mut chain: Blockchain<String> =
            Blockchain::new(ChainParams::fixed_difficulty(difficulty));
        for (i, text) in ["Alice pays Bob 10", "Bob pays Carol 5", "Carol pays Dave 1"]
            .iter()
            .enumerate()
        {
            let (block, _) = mine(chain.next_block(START + i as u64, text.to_string()));
            chain.push(block).unwrap();
        }

        // 攻擊者改了區塊 1，再把它和後面的區塊全部重算雜湊（但不挖礦）。
        let mut forged = chain.blocks().to_vec();
        forged[1].data = "Alice pays Mallory 1000".to_string();
        for i in 1..forged.len() {
            forged[i].header.previous_hash = forged[i - 1].hash.clone();
            forged[i].rehash();
        }
        let statuses = check_blocks(&forged, chain.params());
        if difficulty == 0 {
            assert!(
                statuses.iter().all(|s| *s == BlockStatus::Valid),
                "沒有工作量證明時，偽造的鏈看起來合法"
            );
        } else {
            // 雜湊開頭的 0 不夠：挖礦才找得到合格的 nonce。
            assert!(matches!(
                statuses[1],
                BlockStatus::Invalid(BlockError::InsufficientWork { .. })
            ));
            // 真的重新挖礦之後就合法了，但要付出每個區塊約 2^8 次的計算。
            for i in 1..forged.len() {
                forged[i].header.previous_hash = forged[i - 1].hash.clone();
                forged[i].rehash();
                forged[i] = pow::mine(forged[i].clone()).0;
            }
            assert!(
                check_blocks(&forged, chain.params())
                    .iter()
                    .all(|s| *s == BlockStatus::Valid)
            );
        }
    }
}

/// 第 8 章：網路分成兩邊時，Alice 在兩邊各花了同一筆錢。重新接上後，工作量比較多的那邊勝出，
/// 另一邊的交易放回 mempool，但因為錢已經花掉了，被丟掉。
#[test]
fn double_spend_is_resolved_by_the_heavier_chain() {
    let mut net = Network::new(3, ChainParams::fixed_difficulty(6), 100, START);
    net.connect_all();
    net.run_until_idle(1_000);

    // 節點 0 的礦工（Alice）先挖一個區塊，拿到 50 元。
    net.mine(0);
    net.run_until_idle(1_000);
    let alice = net.node(0).wallet().clone();
    let bob = Wallet::generate().address();
    let carol = Wallet::generate().address();

    // 網路分成 {0} 和 {1, 2}。
    net.disconnect(0, 1);
    net.disconnect(0, 2);

    // 左邊：Alice 付 50 給 Bob，節點 0 挖進區塊。
    net.submit(0, Transaction::transfer(&alice, &bob, 50, START + 1))
        .unwrap();
    net.mine(0);
    // 右邊：Alice 付 50 給 Carol，節點 1、2 連續挖了兩個區塊。
    net.submit(1, Transaction::transfer(&alice, &carol, 50, START + 2))
        .unwrap();
    net.run_until_idle(1_000);
    net.mine(1);
    net.run_until_idle(1_000);
    net.mine(2);
    net.run_until_idle(1_000);
    assert_eq!(net.node(0).ledger().balance(&bob), 50);

    // 重新接上。
    net.connect(0, 1);
    net.run_until_idle(1_000);

    let tips: Vec<_> = net
        .nodes()
        .iter()
        .map(|n| n.chain().latest().hash.clone())
        .collect();
    assert!(tips.iter().all(|t| *t == tips[1]), "大家收斂到同一條鏈");
    let node0 = net.node(0);
    assert_eq!(node0.ledger().balance(&carol), 50);
    assert_eq!(
        node0.ledger().balance(&bob),
        0,
        "Bob 收到的 50 元跟著被換掉的區塊消失了"
    );
    assert!(
        node0.mempool().is_empty(),
        "付給 Bob 的交易放回 mempool 後，因為錢不夠被丟掉"
    );
    assert!(
        net.log()
            .iter()
            .any(|e| e.node == 0 && e.text.contains("換到工作量比較多的鏈"))
    );
}

/// 第 8 章：比的是累積工作量，不是區塊數。
#[test]
fn a_shorter_chain_with_more_work_wins() {
    let easy = ChainParams {
        initial_difficulty: 2,
        min_difficulty: 2,
        max_difficulty: 10,
        target_block_time_ms: 1_000_000,
        adjustment_interval: 2,
        block_reward: 50,
    };
    // 兩個節點用同樣的規則，但出塊的節奏不同：A 慢慢出 6 個區塊，B 很快出 5 個，難度被調高。
    let mut a = Node::new(easy.clone(), Wallet::generate(), None);
    let mut b = Node::new(easy, Wallet::generate(), None);
    for i in 0..6 {
        a.mine(START + i * 10_000_000);
    }
    for i in 0..5 {
        b.mine(START + i);
    }
    assert!(b.chain().len() < a.chain().len());
    assert!(b.chain().total_work() > a.chain().total_work());

    a.handle(blockchain_from_scratch::p2p::Message::ResponseBlockchain {
        blocks: b.blocks().to_vec(),
    });
    assert_eq!(a.chain().latest().hash, b.chain().latest().hash);
}

/// 第 7、9 章：FileChain 在 3 個節點的網路上：從節點 0 上傳，節點 1 出塊，從節點 2 查詢。
#[test]
fn filechain_over_a_network() {
    let chain_key = ChainKey::generate();
    let params = ChainParams::fixed_difficulty(6);
    let nodes = (0..3)
        .map(|_| Node::new(params.clone(), Wallet::generate(), Some(chain_key.clone())))
        .collect();
    let mut net = Network::from_nodes(nodes, 50, START);
    net.connect_all();
    net.run_until_idle(1_000);

    let unit = Wallet::generate();
    let file = b"\x89PNG pretend this is an image".to_vec();
    let upload = Upload {
        file: &file,
        certificate_owner: "Miles",
        cer_level: CerLevel::Medium,
        unit_name: "AMERICA",
        timestamp: START,
    };
    let data = prepare_upload(upload, &unit, &chain_key.public_hex()).unwrap();
    net.submit(0, Transaction::from(data)).unwrap();
    net.run_until_idle(1_000);
    assert_eq!(
        net.node(1).mempool().len(),
        1,
        "檔案紀錄廣播到其他節點的 mempool"
    );

    // 還沒出塊之前查不到。
    assert!(net.node(2).query_by_name("Miles").unwrap().is_empty());
    net.mine(1);
    net.run_until_idle(1_000);

    let hits = net.node(2).query_by_name("Miles").unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(read_hit(&hits[0]).unwrap(), file);
    assert_eq!(hits[0].certifications[0].unit_name, "AMERICA");
    assert_eq!(hits[0].certifications[0].cer_level, CerLevel::Medium);
    assert!(net.nodes().iter().all(|n| n.mempool().is_empty()));
}
