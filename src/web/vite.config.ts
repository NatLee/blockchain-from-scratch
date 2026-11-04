import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// base: "./" 讓同一份 build 放在任何路徑都能用（GitHub Pages 的 /blockchain-from-scratch/、docker 的 /）。
export default defineConfig({
  base: "./",
  plugins: [react(), tailwindcss()],
  server: {
    // 開發時允許讀 repo 裡的 docs/images（範例圖片）。
    fs: { allow: ["..", "../../docs"] },
  },
  build: {
    outDir: "dist",
    target: "es2022",
  },
  test: {
    environment: "jsdom",
    setupFiles: ["test/setup.ts"],
    include: ["test/**/*.test.{ts,tsx}"],
    exclude: ["test/e2e/**"],
  },
});
