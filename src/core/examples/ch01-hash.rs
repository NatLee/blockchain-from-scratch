//! 第 1 章：雜湊。執行：`cargo run --example ch01-hash`

use blockchain_from_scratch::hash::{bit_difference, sha256, sha256_hex};

fn main() {
    println!("== 同樣的輸入，同樣的輸出；改一點點，完全不一樣 ==");
    for text in ["hello", "hello", "hellp", "Hello", "hello "] {
        println!(
            "{:>10} → {}",
            format!("{text:?}"),
            sha256_hex(text.as_bytes())
        );
    }

    println!("\n== 雪崩效應：\"hello\" 和 \"hellp\" 的前 32 個 bit ==");
    let a = sha256(b"hello");
    let b = sha256(b"hellp");
    let bits = |hash: &[u8; 32]| {
        hash[..4]
            .iter()
            .map(|byte| format!("{byte:08b}"))
            .collect::<String>()
    };
    let (bits_a, bits_b) = (bits(&a), bits(&b));
    let marks: String = bits_a
        .chars()
        .zip(bits_b.chars())
        .map(|(x, y)| if x == y { ' ' } else { '^' })
        .collect();
    println!("hello {bits_a}\nhellp {bits_b}\n      {marks}");
    let diff = bit_difference(&a, &b);
    println!(
        "整個 256 bit 裡有 {diff} 個不一樣（{:.0}%），理論上平均是一半。",
        f64::from(diff) / 256.0 * 100.0
    );

    println!("\n== 不管資料多大，雜湊都是 32 bytes（64 個十六進位字元）==");
    let one_megabyte = vec![b'a'; 1_000_000];
    println!("1 MB 的資料 → {}", sha256_hex(&one_megabyte));
}
