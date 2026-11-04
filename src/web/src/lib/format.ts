/** 雜湊太長，畫面上只顯示前後幾個字元。 */
export function short(hash: string | null | undefined, head = 8): string {
  if (!hash) return "—";
  return hash.length > head + 4 ? `${hash.slice(0, head)}…${hash.slice(-4)}` : hash;
}

/** 十六進位轉成 0/1 字串（每個十六進位字元 4 個 bit）。 */
export function hexToBits(hex: string): string {
  return [...hex].map((c) => parseInt(c, 16).toString(2).padStart(4, "0")).join("");
}

/** 用雜湊決定顏色：同一個區塊在每個節點上顏色都一樣，分岔時一眼看得出來。 */
export function hashColor(hash: string): string {
  const hue = parseInt(hash.slice(-6), 16) % 360;
  return `hsl(${hue} 70% 50%)`;
}

export function formatTime(ms: number): string {
  return new Date(ms).toLocaleString("zh-TW", { hour12: false });
}

export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
