import { ArrowRight, BookOpen, Container, Globe, Terminal } from "lucide-react";
import { docUrl } from "../config";

export const CHAPTERS = [
  { n: 1, title: "雜湊", doc: "docs/chapters/01-hash.md", page: "hash", idea: "資料的指紋：改一個字，雜湊就完全不同。" },
  { n: 2, title: "區塊", doc: "docs/chapters/02-block.md", page: "chain", idea: "標頭 + 內容，標頭只放內容的雜湊。" },
  { n: 3, title: "鏈和竄改偵測", doc: "docs/chapters/03-chain.md", page: "chain", idea: "每個區塊記著前一個的雜湊，改一個全部對不上。" },
  { n: 4, title: "工作量證明和難度", doc: "docs/chapters/04-proof-of-work.md", page: "mining", idea: "讓改歷史變得很貴：找開頭有很多 0 的雜湊。" },
  { n: 5, title: "金鑰、簽章和交易", doc: "docs/chapters/05-signatures.md", page: "transactions", idea: "沒有帳號密碼，靠私鑰簽章證明是你。" },
  { n: 6, title: "mempool 和出塊", doc: "docs/chapters/06-mempool.md", page: "transactions", idea: "交易先排隊，礦工打包時順便拿獎勵。" },
  { n: 7, title: "P2P 節點和廣播", doc: "docs/chapters/07-p2p.md", page: "network", idea: "沒有中央伺服器，節點之間互相轉傳。" },
  { n: 8, title: "分岔和共識", doc: "docs/chapters/08-consensus.md", page: "network", idea: "意見不同時，相信累積工作量最多的鏈。" },
  { n: 9, title: "FileChain", doc: "docs/chapters/09-filechain.md", page: "filechain", idea: "把檔案加密、簽章後上鏈，證明它存在過。" },
];

export default function Home() {
  return (
    <div className="space-y-10">
      <section className="space-y-4">
        <p className="font-mono text-sm text-indigo-600 dark:text-indigo-400">blockchain-from-scratch</p>
        <h1 className="text-3xl font-bold tracking-tight text-slate-900 sm:text-4xl dark:text-white">
          從零開始，寫一條自己的區塊鏈
        </h1>
        <p className="max-w-3xl text-lg text-slate-600 dark:text-slate-300">
          九個章節，從雜湊一路做到會廣播、會分岔、會自己收斂的 P2P 網路，最後做出一個檔案存證系統。
          每一章都有可以跑的 Rust 程式、測試和動手做的練習；這個網頁用同一份程式，讓你直接看到每個概念怎麼運作。
        </p>
      </section>

      <section className="grid gap-4 md:grid-cols-3">
        <div className="panel space-y-2">
          <Globe className="size-5 text-indigo-500" />
          <h2 className="font-semibold">在這個網頁上玩</h2>
          <p className="text-sm text-slate-600 dark:text-slate-300">左邊的每一頁都是一個互動示範，不用安裝任何東西。</p>
        </div>
        <div className="panel space-y-2">
          <Terminal className="size-5 text-indigo-500" />
          <h2 className="font-semibold">跑每一章的範例</h2>
          <pre className="overflow-x-auto rounded-lg bg-slate-900 p-3 text-xs text-slate-100">cargo run --example ch01-hash</pre>
        </div>
        <div className="panel space-y-2">
          <Container className="size-5 text-indigo-500" />
          <h2 className="font-semibold">起一個 3 節點的網路</h2>
          <pre className="overflow-x-auto rounded-lg bg-slate-900 p-3 text-xs text-slate-100">docker compose up --build</pre>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold text-slate-900 dark:text-white">學習路線</h2>
        <ol className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {CHAPTERS.map((c) => (
            <li key={c.n} className="panel flex flex-col gap-3">
              <div className="flex items-baseline gap-2">
                <span className="font-mono text-sm text-indigo-600 dark:text-indigo-400">{String(c.n).padStart(2, "0")}</span>
                <h3 className="font-semibold text-slate-900 dark:text-white">{c.title}</h3>
              </div>
              <p className="flex-1 text-sm text-slate-600 dark:text-slate-300">{c.idea}</p>
              <div className="flex gap-2 text-sm">
                <a className="btn-secondary" href={docUrl(c.doc)}>
                  <BookOpen className="size-4" /> 教材
                </a>
                <a className="btn-primary" href={`#/${c.page}`}>
                  試試看 <ArrowRight className="size-4" />
                </a>
              </div>
            </li>
          ))}
        </ol>
      </section>
    </div>
  );
}
