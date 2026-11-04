#!/usr/bin/env sh
# 下載跟 Cargo.lock 裡 wasm-bindgen 同版本的 wasm-bindgen-cli（Linux x86_64 預先編好的版本），
# 檢查 sha256 後放到 $1（預設 /usr/local/bin）。CI 和 Dockerfile 都用這支腳本，版本只記在 Cargo.lock。
set -eu

root=$(cd "$(dirname "$0")/../../.." && pwd)
dest=${1:-/usr/local/bin}
version=$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' "$root/Cargo.lock")
name="wasm-bindgen-$version-x86_64-unknown-linux-musl"
url="https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$version/$name.tar.gz"

work=$(mktemp -d)
curl -fsSL -o "$work/cli.tar.gz" "$url"
expected=$(curl -fsSL "$url.sha256sum" | cut -d' ' -f1)
echo "$expected  $work/cli.tar.gz" | sha256sum -c -
tar xzf "$work/cli.tar.gz" -C "$work"
mkdir -p "$dest"
cp "$work/$name/wasm-bindgen" "$dest/wasm-bindgen"
rm -rf "$work"
"$dest/wasm-bindgen" --version
