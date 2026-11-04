// 網頁用到的網址和預設值，集中放在這裡。

/** GitHub 上的 repo，教材文件的連結從這裡組出來。 */
export const REPO_URL = "https://github.com/NatLee/blockchain-from-scratch";
export const DEFAULT_BRANCH = "master";

/** FileChain 頁面連「真的節點」時的預設網址：docker compose 的節點 1。build 時可以用 VITE_NODE_URL 換掉。 */
export const DEFAULT_NODE_URL: string = import.meta.env.VITE_NODE_URL ?? "http://localhost:3001";

export function docUrl(path: string): string {
  return `${REPO_URL}/blob/${DEFAULT_BRANCH}/${path}`;
}
