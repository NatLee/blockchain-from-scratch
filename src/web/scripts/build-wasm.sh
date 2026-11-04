#!/usr/bin/env sh
# 把 Rust 核心編成 WebAssembly，輸出到 src/web/src/wasm-pkg/（產生的檔案，不進 git）。
#
# 需要：Rust、wasm32 target（rustup target add wasm32-unknown-unknown），
# 以及版本跟 Cargo.lock 裡的 wasm-bindgen 完全一樣的 wasm-bindgen-cli：
#   cargo install wasm-bindgen-cli --locked --version <版本>
# 沒有 Rust 的話，可以用 Docker 跑這支腳本，見 README 的「開發」段落。
set -eu

root=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$root"

version=$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' Cargo.lock)
bindgen=${WASM_BINDGEN:-wasm-bindgen}
installed=$("$bindgen" --version 2>/dev/null | cut -d' ' -f2 || true)
if [ "$installed" != "$version" ]; then
  echo "需要 wasm-bindgen-cli $version（目前是：${installed:-沒有安裝}）" >&2
  echo "安裝：cargo install wasm-bindgen-cli --locked --version $version" >&2
  exit 1
fi

cargo build --locked --release -p chain-wasm --target wasm32-unknown-unknown
target_dir=${CARGO_TARGET_DIR:-$root/target}
"$bindgen" --target web --out-dir src/web/src/wasm-pkg \
  "$target_dir/wasm32-unknown-unknown/release/chain_wasm.wasm"
echo "WebAssembly 輸出到 src/web/src/wasm-pkg/"
