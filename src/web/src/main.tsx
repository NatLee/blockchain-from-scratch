import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { initCore } from "./lib/core";
import "./index.css";

// 先載入 Rust 核心（WebAssembly），載好才載入頁面：有些頁面在模組載入時就會用到核心（例如讀預設參數）。
async function start() {
  await initCore();
  const { default: App } = await import("./App");
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}

void start();
