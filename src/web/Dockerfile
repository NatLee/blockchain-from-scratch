# 互動網頁：Rust 核心編成 WebAssembly → React build → nginx。在 repo 根目錄 build：docker build -f src/web/Dockerfile .

# 1. Rust 核心 → WebAssembly
FROM rust:1-trixie AS wasm
WORKDIR /build
RUN rustup target add wasm32-unknown-unknown
COPY Cargo.toml Cargo.lock ./
COPY src/web/scripts src/web/scripts
RUN sh src/web/scripts/install-wasm-bindgen.sh /usr/local/bin
COPY src/core src/core
COPY src/node src/node
COPY src/wasm src/wasm
RUN sh src/web/scripts/build-wasm.sh

# 2. 網頁
FROM node:24-slim AS web
WORKDIR /repo/src/web
COPY src/web/package.json src/web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY src/web ./
COPY docs/images/samples /repo/docs/images/samples
COPY --from=wasm /build/src/web/src/wasm-pkg ./src/wasm-pkg
# FileChain 頁面「真的節點」的預設網址
ARG VITE_NODE_URL=http://localhost:3001
ENV VITE_NODE_URL=$VITE_NODE_URL
RUN npm run build

# 3. 靜態檔案
FROM nginx:alpine
COPY --from=web /repo/src/web/dist /usr/share/nginx/html
