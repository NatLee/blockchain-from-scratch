// 對真的節點跑：先 `docker compose up -d`，再
// `NODE_URLS=http://localhost:3001,http://localhost:3002,http://localhost:3003 npm run test:e2e`
import { describe, expect, it } from "vitest";
import { core } from "../../src/lib/core";
import { searchFiles, uploadFile } from "../../src/lib/filechain";
import { HttpNodeApi, isBlock } from "../../src/lib/nodeApi";
import { sampleImage } from "../helpers";

const urls = (process.env.NODE_URLS ?? "").split(",").filter(Boolean);

async function eventually(what: string, check: () => Promise<boolean>) {
  for (let i = 0; i < 100; i++) {
    if (await check()) return;
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`等不到：${what}`);
}

describe.skipIf(urls.length < 3)("FileChain on a real 3-node network", () => {
  it("uploads on node 1, mines on node 2, finds it on node 3", async () => {
    const [n1, n2, n3] = urls.map((u) => new HttpNodeApi(u));
    await eventually("節點 1 連到其他節點", async () => (await n1.peers()).length >= 2);

    const owner = `e2e-${Date.now()}`;
    const image = sampleImage("sample-sunrise.png");
    await uploadFile(n1, image, owner, "Medium", { name: "AMERICA", wallet: core.generateWallet() });
    await eventually("檔案廣播到節點 3", async () => (await n3.poolList()).length > 0);

    const block = await n2.mineBlock();
    expect(isBlock(block)).toBe(true);
    const height = isBlock(block) ? block.index : -1;
    for (const node of [n1, n3]) {
      await eventually("區塊傳到其他節點", async () => (await node.blocks()).length > height);
    }

    const found = await searchFiles(n3, owner);
    expect(found).toHaveLength(1);
    expect(found[0].bytes).toEqual(image);
    expect(found[0].hit.certifications[0]).toMatchObject({ unitName: "AMERICA", cerLevel: "Medium", count: 1 });
  });
});
