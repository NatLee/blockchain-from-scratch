import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { initCoreSync } from "../src/lib/core";

// 測試在 Node.js 裡跑：直接讀 .wasm 檔初始化 Rust 核心（瀏覽器裡是用 fetch 載入）。
// 用路徑字串而不是 URL：jsdom 環境裡的 URL 是 jsdom 的版本，Node.js 的 fs 不認得。
initCoreSync({ module: readFileSync(resolve(process.cwd(), "src/wasm-pkg/chain_wasm_bg.wasm")) });

// jsdom 沒有實作 createObjectURL；畫面測試只需要它回傳一個字串。
if (typeof URL.createObjectURL !== "function") {
  URL.createObjectURL = () => "blob:test";
  URL.revokeObjectURL = () => {};
}

// jsdom 也沒有 scrollTo（換頁時會呼叫）。
if (typeof window !== "undefined") window.scrollTo = () => {};
