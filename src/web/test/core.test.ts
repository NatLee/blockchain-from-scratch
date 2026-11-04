import { describe, expect, it } from "vitest";
import { core } from "../src/lib/core";
import type { ChainParams, TextBlock } from "../src/lib/types";

const fixed = (d: number): ChainParams => ({ ...core.defaultParams(), initialDifficulty: d, minDifficulty: d, maxDifficulty: d });

function mine(block: TextBlock): TextBlock {
  let step = core.mineSteps(block, 1_000);
  while (!step.found) step = core.mineSteps(step.block, 1_000);
  return step.block;
}

describe("第 1 章：雜湊（WebAssembly 跑的是同一份 Rust）", () => {
  it("matches the FIPS 180-2 test vector", () => {
    expect(core.sha256Hex("abc")).toBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    expect(core.sha256BytesHex(new TextEncoder().encode("abc"))).toBe(core.sha256Hex("abc"));
  });

  it("shows the avalanche effect", () => {
    const diff = core.bitDifference(core.sha256Hex("hello"), core.sha256Hex("hellp"));
    expect(diff).toBeGreaterThan(80);
    expect(diff).toBeLessThan(176);
  });

  it("throws a readable error for bad input", () => {
    expect(() => core.bitDifference("zz", "zz")).toThrow(/64 個十六進位/);
  });
});

describe("第 2–4 章：區塊、鏈和挖礦", () => {
  const params = fixed(8);
  const chain = (() => {
    const blocks = [core.genesisBlock()];
    for (const data of ["a", "b", "c"]) blocks.push(mine(core.nextBlock(blocks, params, 1.7e12, data)));
    return blocks;
  })();

  it("a mined chain is valid and hashes meet the difficulty", () => {
    expect(core.checkTextChain(chain, params).map((s) => s.status)).toEqual(["valid", "valid", "valid", "valid"]);
    expect(core.leadingZeroBits(chain[1].hash)).toBeGreaterThanOrEqual(8);
  });

  it("tampering turns the block red and everything after amber", () => {
    const tampered = chain.map((b, i) => (i === 1 ? { ...b, data: "A" } : b));
    const statuses = core.checkTextChain(tampered, params);
    expect(statuses[1]).toEqual({ status: "invalid", reason: "data_hash 對不上內容：內容被改過" });
    expect(statuses[2].status).toBe("buildsOnInvalid");
  });

  it("rehashing without mining usually fails the proof of work", () => {
    const rehashed = chain.map((b, i) => (i === 1 ? core.rehashBlock({ ...b, data: "A", nonce: 0 }) : b));
    const statuses = core.checkTextChain(rehashed, params);
    // 大約 1/256 的機率 nonce 0 剛好合格；那樣的話區塊 1 正常，但區塊 2 一定接不上。
    if (statuses[1].status === "invalid") expect(statuses[1].reason).toMatch(/0 不夠/);
    else expect(statuses[2]).toEqual({ status: "invalid", reason: "previous_hash 對不上前一個區塊的 hash" });
  });

  it("work doubles with difficulty and the hash input is the header", () => {
    expect(core.blockWork(10)).toBe(1024n);
    expect(core.hashInput(chain[1])).toContain(`|${chain[1].dataHash}|8|`);
  });
});

describe("第 5 章：簽章", () => {
  it("verifies a signed transfer and rejects a changed amount", () => {
    const alice = core.generateWallet();
    const bob = core.generateWallet();
    expect(core.addressOf(alice.secret)).toBe(alice.address);
    expect(core.isAddress(bob.address)).toBe(true);
    const tx = core.signTransfer(alice.secret, bob.address, 10, 1);
    expect(core.verifyTransaction(tx).ok).toBe(true);
    if (tx.type !== "transfer") throw new Error("expected a transfer");
    const forged = { ...tx, body: { ...tx.body, amount: 1000 } };
    expect(core.verifyTransaction(forged)).toMatchObject({ ok: false, error: expect.stringMatching(/簽章不對/) });
  });
});
