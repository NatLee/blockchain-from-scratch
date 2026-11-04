// FileChain 客戶端用的節點介面。兩種實作，回傳的 JSON 格式一模一樣：
// - BrowserNodeApi：節點跑在這個分頁裡（Rust 核心編成的 WebAssembly），不用安裝任何東西。
// - HttpNodeApi：連到真的節點（docker compose 起的 3 個節點），用原本的 HTTP API。
import { BrowserNode } from "./core";
import type { AddData, ChainParams, FileHit, StatusResponse, Transaction, TxBlock } from "./types";

export interface NodeApi {
  readonly label: string;
  getPublicKey(): Promise<string>;
  addData(data: AddData): Promise<Transaction[]>;
  poolList(): Promise<Transaction[]>;
  mineBlock(): Promise<TxBlock | StatusResponse>;
  blocks(): Promise<TxBlock[]>;
  queryBlockByName(name: string): Promise<FileHit[]>;
  peers(): Promise<string[]>;
}

export function isBlock(value: TxBlock | StatusResponse): value is TxBlock {
  return "hash" in value;
}

export class BrowserNodeApi implements NodeApi {
  readonly label = "瀏覽器裡的節點";
  private node: BrowserNode;

  constructor(params: ChainParams) {
    this.node = new BrowserNode(JSON.stringify(params));
  }

  async getPublicKey() {
    return (JSON.parse(this.node.getPublicKey()) as { public_key: string }).public_key;
  }
  async addData(data: AddData) {
    return JSON.parse(this.node.addData(JSON.stringify(data))) as Transaction[];
  }
  async poolList() {
    return JSON.parse(this.node.poolList()) as Transaction[];
  }
  async mineBlock() {
    // 挖礦會占住這個分頁一下子（難度 16 大約幾十毫秒）。
    return JSON.parse(this.node.mineBlock(Date.now())) as TxBlock | StatusResponse;
  }
  async blocks() {
    return JSON.parse(this.node.blocks()) as TxBlock[];
  }
  async queryBlockByName(name: string) {
    return JSON.parse(this.node.queryBlockByName(name)) as FileHit[];
  }
  async peers() {
    return JSON.parse(this.node.peers()) as string[];
  }
}

export class HttpNodeApi implements NodeApi {
  readonly label: string;

  constructor(private baseUrl: string) {
    this.baseUrl = baseUrl.replace(/\/+$/, "");
    this.label = this.baseUrl;
  }

  private async request<T>(path: string, init?: RequestInit): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, init);
    const body = await response.json().catch(() => null);
    if (!response.ok) {
      const message = (body as StatusResponse | null)?.message ?? `HTTP ${response.status}`;
      throw new Error(message);
    }
    return body as T;
  }

  private post<T>(path: string, body: unknown) {
    return this.request<T>(path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
  }

  async getPublicKey() {
    return (await this.request<{ public_key: string }>("/getPublicKey")).public_key;
  }
  addData(data: AddData) {
    return this.post<Transaction[]>("/addData", data);
  }
  poolList() {
    return this.request<Transaction[]>("/poolList");
  }
  mineBlock() {
    return this.post<TxBlock | StatusResponse>("/mineBlock", {});
  }
  blocks() {
    return this.request<TxBlock[]>("/blocks");
  }
  queryBlockByName(name: string) {
    return this.request<FileHit[]>(`/queryBlockByName?queryName=${encodeURIComponent(name)}`);
  }
  peers() {
    return this.request<string[]>("/peers");
  }
}
