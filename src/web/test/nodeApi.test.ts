import { describe, expect, it } from "vitest";
import { core } from "../src/lib/core";
import { searchFiles, uploadFile } from "../src/lib/filechain";
import { BrowserNodeApi, isBlock } from "../src/lib/nodeApi";
import { sampleImage } from "./helpers";

const params = { ...core.defaultParams(), initialDifficulty: 8, minDifficulty: 8 };

describe("第 9 章：FileChain（瀏覽器裡的節點，跟 HTTP API 同樣的格式）", () => {
  it("uploads, mines and finds the same image", async () => {
    const api = new BrowserNodeApi(params);
    const image = sampleImage();
    const america = { name: "AMERICA", wallet: core.generateWallet() };
    const china = { name: "CHINA", wallet: core.generateWallet() };

    expect(await api.getPublicKey()).toMatch(/^[0-9a-f]{64}$/);
    await uploadFile(api, image, "Miles", "High", america);
    const pool = await uploadFile(api, image, "Miles", "Low", china);
    expect(pool).toHaveLength(2);
    expect(await searchFiles(api, "Miles")).toEqual([]);

    const block = await api.mineBlock();
    expect(isBlock(block) && block.index).toBe(1);
    expect(await api.poolList()).toEqual([]);
    expect(await api.blocks()).toHaveLength(2);

    const found = await searchFiles(api, "Miles");
    expect(found).toHaveLength(1);
    expect(found[0].bytes).toEqual(image);
    expect(found[0].hit.certifications.map((c) => [c.unitName, c.cerLevel])).toEqual([
      ["CHINA", "Low"],
      ["AMERICA", "High"],
    ]);
  });

  it("keeps the original empty-pool answer and rejects forged uploads", async () => {
    const api = new BrowserNodeApi(params);
    expect(await api.mineBlock()).toEqual({ status: false, message: "No data can be packed into block" });
    const unit = core.generateWallet();
    const data = core.prepareUpload(sampleImage(), "Miles", "None", "AMERICA", unit.secret, await api.getPublicKey(), 1);
    await expect(api.addData({ ...data, certificateOwner: "Mallory" })).rejects.toThrow(/簽章不對/);
    expect(await api.peers()).toEqual([]);
  });
});
