# 區塊鏈節點（Rust）。在 repo 根目錄 build：docker build -f src/node/Dockerfile .
FROM rust:1-trixie AS build
WORKDIR /build
# workspace 的每個成員都要在，cargo 才讀得懂 Cargo.toml；只會編譯節點用到的部分。
COPY Cargo.toml Cargo.lock ./
COPY src/core src/core
COPY src/node src/node
COPY src/wasm src/wasm
RUN cargo build --release --locked -p chain-node

FROM debian:trixie-slim
# /keys 放 FileChain 的鏈金鑰（第一次啟動時產生）。docker compose 用 volume 讓 3 個節點共用。
RUN useradd --uid 10001 --create-home --shell /usr/sbin/nologin app \
    && mkdir -p /keys \
    && chown app /keys
COPY --from=build /build/target/release/chain-node /usr/local/bin/chain-node
ENV PORT=3001 \
    CHAIN_KEY_FILE=/keys/chain-key.hex \
    RUST_LOG=info
USER app
EXPOSE 3001
CMD ["chain-node"]
