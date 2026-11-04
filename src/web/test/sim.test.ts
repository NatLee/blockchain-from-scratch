import { describe, expect, it } from "vitest";
import { SimNetwork, core } from "../src/lib/core";
import type { SimSnapshot } from "../src/lib/types";

const params = JSON.stringify({ ...core.defaultParams(), initialDifficulty: 6, minDifficulty: 6, maxDifficulty: 6 });
const snap = (net: SimNetwork): SimSnapshot => JSON.parse(net.snapshot());

describe("第 7–8 章：模擬網路", () => {
  it("broadcasts a block and resolves a double spend by total work", () => {
    const net = new SimNetwork(3, params, 100, 1.7e12);
    net.connectAll();
    net.mine(0);
    net.runUntilIdle();
    expect(snap(net).nodes.map((n) => n.height)).toEqual([1, 1, 1]);

    const bob = core.generateWallet().address;
    const carol = core.generateWallet().address;
    net.disconnect(0, 1);
    net.disconnect(0, 2);
    net.transfer(0, 0, bob, 50, 1);
    net.mine(0);
    net.transfer(1, 0, carol, 50, 2);
    net.runUntilIdle();
    net.mine(1);
    net.runUntilIdle();
    net.mine(2);
    net.runUntilIdle();
    net.connect(0, 1);
    net.runUntilIdle();

    const after = snap(net);
    expect(new Set(after.nodes.map((n) => n.tipHash)).size).toBe(1);
    expect(after.nodes[0].balances[carol]).toBe(50);
    expect(after.nodes[0].balances[bob]).toBeUndefined();
    expect(after.log.some((e) => e.text.includes("換到工作量比較多的鏈"))).toBe(true);
    net.free();
  });

  it("rejects spending more than the miner has", () => {
    const net = new SimNetwork(1, params, 0, 1.7e12);
    expect(() => net.transfer(0, 0, core.generateWallet().address, 1, 1)).toThrow(/餘額不足/);
    net.free();
  });
});
