// FileChain 客戶端的流程（原本 Django 網站 imgTest/views.py 做的事），全部交給 Rust 核心處理。
import { core } from "./core";
import type { NodeApi } from "./nodeApi";
import type { CerLevel, FileHit, Transaction, Wallet } from "./types";

export interface Unit {
  name: string;
  wallet: Wallet;
}

/** 上傳：向節點拿鏈的公鑰 → 壓縮、加密、簽章（Rust）→ POST /addData。回傳上傳後的 pool。 */
export async function uploadFile(
  api: NodeApi,
  file: Uint8Array,
  certificateOwner: string,
  cerLevel: CerLevel,
  unit: Unit,
): Promise<Transaction[]> {
  const chainPublicKey = await api.getPublicKey();
  const data = core.prepareUpload(file, certificateOwner, cerLevel, unit.name, unit.wallet.secret, chainPublicKey, Date.now());
  return api.addData(data);
}

export interface FoundFile {
  hit: FileHit;
  bytes?: Uint8Array;
  error?: string;
}

/** 查詢：GET /queryBlockByName → 解壓縮並確認雜湊（Rust）。 */
export async function searchFiles(api: NodeApi, name: string): Promise<FoundFile[]> {
  const hits = await api.queryBlockByName(name);
  return hits.map((hit) => {
    if (hit.error) return { hit, error: hit.error };
    try {
      return { hit, bytes: core.readHit(hit) };
    } catch (e) {
      return { hit, error: e instanceof Error ? e.message : String(e) };
    }
  });
}

/** 從檔案開頭的幾個 bytes 判斷是不是圖片（瀏覽器要知道類型才會顯示）。 */
export function imageType(bytes: Uint8Array): string | null {
  const starts = (...sig: number[]) => sig.every((b, i) => bytes[i] === b);
  if (starts(0x89, 0x50, 0x4e, 0x47)) return "image/png";
  if (starts(0xff, 0xd8, 0xff)) return "image/jpeg";
  if (starts(0x47, 0x49, 0x46, 0x38)) return "image/gif";
  if (starts(0x52, 0x49, 0x46, 0x46) && bytes[8] === 0x57 && bytes[9] === 0x45) return "image/webp";
  return null;
}
