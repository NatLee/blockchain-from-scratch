//! 第 5 章：金鑰、簽章和交易。執行：`cargo run --example ch05-signature`

use blockchain_from_scratch::transaction::Transaction;
use blockchain_from_scratch::wallet::Wallet;

fn main() {
    let alice = Wallet::generate();
    let bob = Wallet::generate();
    let mallory = Wallet::generate();
    println!("Alice 的地址（公鑰）：{}", alice.address());
    println!("Bob 的地址（公鑰）：  {}", bob.address());

    let tx = Transaction::transfer(&alice, &bob.address(), 10, 1_700_000_000_000);
    println!("\n== Alice 簽了一筆轉帳 ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&tx).expect("transactions serialize to JSON")
    );
    println!("驗證簽章：{:?}", tx.verify_signature());
    println!("交易編號：{}", tx.id());

    println!("\n== Mallory 把金額改成 1000 ==");
    let Transaction::Transfer(mut forged) = tx.clone() else {
        unreachable!("we just made a transfer")
    };
    forged.body.amount = 1_000;
    println!(
        "驗證簽章：{:?}",
        Transaction::Transfer(forged).verify_signature()
    );

    println!("\n== Mallory 自己簽，卻把付款人寫成 Alice ==");
    let Transaction::Transfer(mut impostor) =
        Transaction::transfer(&mallory, &mallory.address(), 10, 1)
    else {
        unreachable!("we just made a transfer")
    };
    impostor.public_key = alice.address();
    println!(
        "驗證簽章：{:?}",
        Transaction::Transfer(impostor).verify_signature()
    );
}
