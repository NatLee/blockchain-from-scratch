import { defineConfig } from "vitest/config";

// 對真的節點跑的測試（docker compose 起 3 個節點之後）：NODE_URLS=http://localhost:3001,… npm run test:e2e
export default defineConfig({
  test: {
    environment: "node",
    setupFiles: ["test/setup.ts"],
    include: ["test/e2e/**/*.test.ts"],
    testTimeout: 60_000,
  },
});
