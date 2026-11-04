//! 教材文件的檢查：文件裡的解答要跟有測試的解答一模一樣，範例要存在，相對連結不能壞掉。
//! 這樣改程式時忘了改文件（或反過來），`cargo test` 就會提醒。

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// 要檢查的 Markdown 檔：README、docs/ 底下的文件和各章。
fn markdown_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = vec![
        root.join("README.md"),
        root.join("docs/original-project.md"),
    ];
    let mut chapters: Vec<PathBuf> = fs::read_dir(root.join("docs/chapters"))
        .expect("docs/chapters exists")
        .map(|entry| entry.expect("readable entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    chapters.sort();
    assert_eq!(chapters.len(), 9, "九章教材都要在");
    files.extend(chapters);
    files
}

/// 把程式碼的每一行去掉前後空白，方便比對（排版不同也算一樣）。
fn trimmed_lines(code: &str) -> Vec<String> {
    code.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn solutions_in_the_docs_are_the_tested_ones() {
    let tested = trimmed_lines(include_str!("exercises.rs"));
    let mut checked = 0;
    for path in markdown_files() {
        let text = fs::read_to_string(&path).unwrap();
        // 每個 `<!-- solution -->` 標記後面的 ```rust 區塊。
        for (_, rest) in text
            .match_indices("<!-- solution -->")
            .map(|(i, _)| text.split_at(i))
        {
            let fence = rest
                .find("```rust")
                .expect("a rust block follows the marker");
            // 從 ```rust 那一行的下一行開始（Windows 的換行也可以）。
            let start = fence + rest[fence..].find('\n').expect("the fence line ends") + 1;
            let end = start + rest[start..].find("```").expect("the rust block is closed");
            let snippet = trimmed_lines(&rest[start..end]);
            let found = tested
                .windows(snippet.len())
                .any(|window| window == snippet.as_slice());
            assert!(
                found,
                "{} 的解答跟 tests/exercises.rs 不一樣：\n{}",
                path.display(),
                snippet.join("\n")
            );
            checked += 1;
        }
    }
    assert!(checked >= 9, "至少每章一個解答，現在只有 {checked} 個");
}

#[test]
fn every_example_mentioned_exists() {
    let root = repo_root();
    for path in markdown_files() {
        let text = fs::read_to_string(&path).unwrap();
        for (i, _) in text.match_indices("--example ") {
            let name: String = text[i + "--example ".len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            // 例如 README 裡的 `--example <名稱>`，不是真的範例名稱。
            if name.is_empty() {
                continue;
            }
            let in_core = root.join(format!("src/core/examples/{name}.rs"));
            let in_node = root.join(format!("src/node/examples/{name}.rs"));
            assert!(
                in_core.exists() || in_node.exists(),
                "{} 提到的範例 {name} 不存在",
                path.display()
            );
        }
    }
}

#[test]
fn relative_links_point_to_real_files() {
    for path in markdown_files() {
        let text = fs::read_to_string(&path).unwrap();
        let dir = path.parent().unwrap();
        for (i, _) in text.match_indices("](") {
            let target: String = text[i + 2..].chars().take_while(|c| *c != ')').collect();
            let target = target.split('#').next().unwrap_or_default();
            if target.is_empty() || target.contains("://") || target.starts_with("mailto:") {
                continue;
            }
            assert!(
                dir.join(target).exists(),
                "{} 的連結 {target} 指到不存在的檔案",
                path.display()
            );
        }
    }
}
