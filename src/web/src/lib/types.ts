// Rust 核心回傳的 JSON 長什麼樣子。欄位名稱跟 Rust 的 serde 設定一致。

export interface ChainParams {
  initialDifficulty: number;
  minDifficulty: number;
  maxDifficulty: number;
  targetBlockTimeMs: number;
  adjustmentInterval: number;
  blockReward: number;
}

export interface Block<D> {
  index: number;
  previousHash: string;
  timestamp: number;
  dataHash: string;
  difficulty: number;
  nonce: number;
  hash: string;
  data: D;
}

export type TextBlock = Block<string>;

export type BlockStatus =
  | { status: "valid" }
  | { status: "invalid"; reason: string }
  | { status: "buildsOnInvalid" };

export interface MineStep {
  found: boolean;
  block: TextBlock;
  attempts: number;
}

export interface Wallet {
  secret: string;
  address: string;
}

export type CerLevel = "None" | "Low" | "Medium" | "High";
export const CER_LEVELS: CerLevel[] = ["None", "Low", "Medium", "High"];

export interface Signed<T> {
  body: T;
  publicKey: string;
  signature: string;
}

export interface Transfer {
  to: string;
  amount: number;
  timestamp: number;
}

export interface FileRecord {
  certificateOwner: string;
  base64Data: string;
  fileHash: string;
  cerLevel: CerLevel;
  unitName: string;
  timestamp: number;
}

export type Transaction =
  | { type: "coinbase"; to: string; amount: number; height: number }
  | ({ type: "transfer" } & Signed<Transfer>)
  | ({ type: "file" } & Signed<FileRecord>);

export type TxBlock = Block<Transaction[]>;

/** `POST /addData` 的內容，欄位名稱沿用原本的 API。 */
export interface AddData {
  base64Data: string;
  signature: string;
  public_key: string;
  certificateOwner: string;
  fileHash: string;
  cerLevel: CerLevel;
  unitName: string;
  timestamp: number;
}

export interface Certification {
  publicKey: string;
  unitName: string;
  cerLevel: CerLevel;
  count: number;
}

export interface FileHit {
  fileHash: string;
  gzipData: string | null;
  error?: string;
  certifications: Certification[];
}

export interface StatusResponse {
  status: boolean;
  message?: string;
}

export interface VerifyResult {
  ok: boolean;
  error?: string;
  id: string;
}

// ── 模擬網路（SimNetwork.snapshot）──

export interface TxSummary {
  id: string;
  type: "coinbase" | "transfer" | "file";
  from?: string;
  to?: string;
  amount?: number;
  certificateOwner?: string;
}

export interface BlockSummary {
  index: number;
  hash: string;
  previousHash: string;
  timestamp: number;
  difficulty: number;
  miner: string | null;
  transactions: TxSummary[];
}

export interface SimNode {
  index: number;
  address: string;
  height: number;
  tipHash: string;
  totalWork: string;
  nextDifficulty: number;
  blocks: BlockSummary[];
  mempool: TxSummary[];
  balances: Record<string, number>;
}

export interface InFlight {
  deliverAt: number;
  from: number;
  to: number;
  summary: string;
}

export interface LogEntry {
  time: number;
  node: number;
  text: string;
}

export interface SimSnapshot {
  now: number;
  delivered: number;
  links: [number, number][];
  inFlight: InFlight[];
  log: LogEntry[];
  nodes: SimNode[];
}
