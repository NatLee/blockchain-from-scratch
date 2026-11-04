import { readFileSync } from "node:fs";
import { resolve } from "node:path";

/** repo 裡自己做的範例圖片（docs/images/samples）。 */
export function sampleImage(name = "sample-certificate.png"): Uint8Array {
  // 測試從 src/web 執行。
  return new Uint8Array(readFileSync(resolve(process.cwd(), "../../docs/images/samples", name)));
}
