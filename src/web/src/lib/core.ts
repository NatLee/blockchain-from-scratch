// 教材的 Rust 核心（編成 WebAssembly）。這裡只把 JSON 字串轉成 TypeScript 型別，邏輯都在 Rust 裡。
import * as wasm from "../wasm-pkg/chain_wasm";
import type {
  AddData,
  BlockStatus,
  CerLevel,
  ChainParams,
  FileHit,
  MineStep,
  TextBlock,
  Transaction,
  VerifyResult,
  Wallet,
} from "./types";

export { BrowserNode, SimNetwork } from "../wasm-pkg/chain_wasm";
export { default as initCore, initSync as initCoreSync } from "../wasm-pkg/chain_wasm";

const json = JSON.stringify;

export const core = {
  // 第 1 章
  sha256Hex: (text: string) => wasm.sha256Hex(text),
  sha256BytesHex: (bytes: Uint8Array) => wasm.sha256BytesHex(bytes),
  bitDifference: (a: string, b: string) => wasm.bitDifference(a, b),
  leadingZeroBits: (hash: string) => wasm.leadingZeroBits(hash),

  // 第 2–4 章
  defaultParams: (): ChainParams => JSON.parse(wasm.defaultParams()),
  genesisBlock: (): TextBlock => JSON.parse(wasm.genesisBlock()),
  nextBlock: (blocks: TextBlock[], params: ChainParams, timestamp: number, data: string): TextBlock =>
    JSON.parse(wasm.nextBlock(json(blocks), json(params), timestamp, data)),
  rehashBlock: (block: TextBlock): TextBlock => JSON.parse(wasm.rehashBlock(json(block))),
  hashInput: (block: TextBlock) => wasm.hashInput(json(block)),
  mineSteps: (block: TextBlock, maxAttempts: number): MineStep => JSON.parse(wasm.mineSteps(json(block), maxAttempts)),
  checkTextChain: (blocks: TextBlock[], params: ChainParams): BlockStatus[] =>
    JSON.parse(wasm.checkTextChain(json(blocks), json(params))),
  nextDifficulty: (blocks: TextBlock[], params: ChainParams) => wasm.nextDifficulty(json(blocks), json(params)),
  blockWork: (difficulty: number) => BigInt(wasm.blockWork(difficulty)),

  // 第 5 章
  generateWallet: (): Wallet => JSON.parse(wasm.generateWallet()),
  addressOf: (secret: string) => wasm.addressOf(secret),
  isAddress: (address: string) => wasm.isAddress(address),
  signTransfer: (secret: string, to: string, amount: number, timestamp: number): Transaction =>
    JSON.parse(wasm.signTransfer(secret, to, amount, timestamp)),
  verifyTransaction: (tx: unknown): VerifyResult => JSON.parse(wasm.verifyTransaction(json(tx))),

  // 第 9 章
  prepareUpload: (
    file: Uint8Array,
    certificateOwner: string,
    cerLevel: CerLevel,
    unitName: string,
    unitSecret: string,
    chainPublicKey: string,
    timestamp: number,
  ): AddData =>
    JSON.parse(wasm.prepareUpload(file, certificateOwner, cerLevel, unitName, unitSecret, chainPublicKey, timestamp)),
  readHit: (hit: FileHit): Uint8Array => wasm.readHit(json(hit)),
};

/** 分段挖礦：每試 `batch` 次就讓瀏覽器喘口氣（更新畫面），`signal` 可以中途取消。 */
export async function mineAsync(
  block: TextBlock,
  onProgress: (attempts: number, block: TextBlock) => void,
  signal?: AbortSignal,
  batch = 20_000,
): Promise<{ block: TextBlock; attempts: number } | null> {
  let current = block;
  let attempts = 0;
  for (;;) {
    if (signal?.aborted) return null;
    const step = core.mineSteps(current, batch);
    attempts += step.attempts;
    current = step.block;
    onProgress(attempts, current);
    if (step.found) return { block: current, attempts };
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}
