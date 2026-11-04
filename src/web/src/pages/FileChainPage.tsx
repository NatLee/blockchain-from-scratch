import { FileCheck2, ImageUp, KeyRound, Pickaxe, RefreshCw, Search, Server, ShieldCheck, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import certificateUrl from "../../../../docs/images/samples/sample-certificate.png?url";
import patternUrl from "../../../../docs/images/samples/sample-pattern.png?url";
import sunriseUrl from "../../../../docs/images/samples/sample-sunrise.png?url";
import { Field, Notice, PageHeader, Panel } from "../components/ui";
import { DEFAULT_NODE_URL } from "../config";
import { core } from "../lib/core";
import { imageType, searchFiles, uploadFile, type FoundFile, type Unit } from "../lib/filechain";
import { errorMessage, short } from "../lib/format";
import { BrowserNodeApi, HttpNodeApi, isBlock, type NodeApi } from "../lib/nodeApi";
import { CER_LEVELS, type CerLevel, type Transaction, type TxBlock } from "../lib/types";

const SAMPLES = [
  { name: "sample-certificate.png", url: certificateUrl },
  { name: "sample-sunrise.png", url: sunriseUrl },
  { name: "sample-pattern.png", url: patternUrl },
];
/** 原本網頁下拉選單顯示的文字。 */
const LEVEL_LABEL: Record<CerLevel, string> = { None: "NONE", Low: "LOW", Medium: "MEDIUM", High: "HIGH" };
/** 瀏覽器裡的節點用比較低的難度，挖礦不會讓畫面卡住。 */
const BROWSER_PARAMS = { ...core.defaultParams(), initialDifficulty: 14, minDifficulty: 14 };

type Mode = "browser" | "http";
type Message = { kind: "ok" | "error" | "info"; text: string };

function useObjectUrl(bytes: Uint8Array | undefined, type: string | null) {
  const url = useMemo(() => (bytes && type ? URL.createObjectURL(new Blob([bytes as BlobPart], { type })) : null), [bytes, type]);
  useEffect(() => () => {
    if (url) URL.revokeObjectURL(url);
  }, [url]);
  return url;
}

function FoundCard({ found, nameOfKey }: { found: FoundFile; nameOfKey: (key: string) => string }) {
  const type = found.bytes ? imageType(found.bytes) : null;
  const url = useObjectUrl(found.bytes, type);
  return (
    <article className="overflow-hidden rounded-xl border border-slate-200 dark:border-slate-800">
      <div className="flex aspect-[3/2] items-center justify-center bg-slate-100 dark:bg-slate-950">
        {url ? (
          <img src={url} alt={`查到的檔案 ${short(found.hit.fileHash)}`} className="max-h-full max-w-full object-contain" />
        ) : (
          <span className="text-sm text-slate-500">{found.bytes ? `${found.bytes.length} bytes 的檔案（不是圖片）` : found.error}</span>
        )}
      </div>
      <div className="space-y-2 p-3 text-sm">
        <p className="flex items-center gap-1 text-xs">
          {found.bytes ? (
            <>
              <ShieldCheck className="size-4 text-emerald-600" aria-hidden />
              <span className="text-emerald-700 dark:text-emerald-300">解壓縮後的雜湊跟 fileHash 一樣</span>
            </>
          ) : (
            <span className="text-rose-600">{found.error}</span>
          )}
        </p>
        <p className="hash text-slate-500" title={found.hit.fileHash}>
          fileHash {short(found.hit.fileHash, 16)}
        </p>
        <p className="label">驗證過的單位</p>
        <ul className="space-y-1">
          {found.hit.certifications.map((c) => (
            <li key={`${c.publicKey}-${c.cerLevel}`} className="flex items-center justify-between gap-2 rounded-lg bg-slate-50 px-2 py-1 dark:bg-slate-900">
              <span>
                <span className="font-medium">{c.unitName || "（沒有名稱）"}</span>{" "}
                <span className="text-xs text-slate-500">{nameOfKey(c.publicKey)}</span>
              </span>
              <span className="text-xs whitespace-nowrap">
                {LEVEL_LABEL[c.cerLevel]} × {c.count}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </article>
  );
}

export default function FileChainPage() {
  const [mode, setMode] = useState<Mode>("browser");
  const [nodeUrl, setNodeUrl] = useState(DEFAULT_NODE_URL);
  const browserNode = useRef<BrowserNodeApi | null>(null);
  if (!browserNode.current) browserNode.current = new BrowserNodeApi(BROWSER_PARAMS);
  const api: NodeApi = useMemo(() => (mode === "browser" ? browserNode.current! : new HttpNodeApi(nodeUrl)), [mode, nodeUrl]);

  const [unit, setUnit] = useState<Unit>(() => ({ name: "AMERICA", wallet: core.generateWallet() }));
  const [file, setFile] = useState<{ name: string; bytes: Uint8Array; url: string } | null>(null);
  const [owner, setOwner] = useState("Miles");
  const [level, setLevel] = useState<CerLevel>("None");
  const [query, setQuery] = useState("Miles");
  const [results, setResults] = useState<FoundFile[] | null>(null);
  const [pool, setPool] = useState<Transaction[]>([]);
  const [blocks, setBlocks] = useState<TxBlock[]>([]);
  const [peers, setPeers] = useState<string[]>([]);
  const [message, setMessage] = useState<Message | null>(null);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [p, b, peerList] = await Promise.all([api.poolList(), api.blocks(), api.peers()]);
      setPool(p);
      setBlocks(b);
      setPeers(peerList);
    } catch (e) {
      setMessage({ kind: "error", text: `連不上節點 ${api.label}：${errorMessage(e)}。docker compose 起好了嗎？` });
    }
  }, [api]);

  useEffect(() => {
    setResults(null);
    void refresh();
  }, [refresh]);

  const run = async (action: () => Promise<Message | null>) => {
    setBusy(true);
    try {
      const result = await action();
      if (result) setMessage(result);
    } catch (e) {
      setMessage({ kind: "error", text: errorMessage(e) });
    }
    setBusy(false);
    await refresh();
  };

  const choose = async (name: string, data: Blob) => {
    const bytes = new Uint8Array(await data.arrayBuffer());
    if (file) URL.revokeObjectURL(file.url);
    setFile({ name, bytes, url: URL.createObjectURL(new Blob([bytes as BlobPart], { type: imageType(bytes) ?? "application/octet-stream" })) });
  };

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragging(false);
    const dropped = e.dataTransfer.files[0];
    if (dropped) void choose(dropped.name, dropped);
  };

  const upload = () =>
    run(async () => {
      if (!file) return { kind: "error", text: "先選一張圖片。" };
      const newPool = await uploadFile(api, file.bytes, owner, level, unit);
      return { kind: "ok", text: `上傳成功：節點驗證簽章後放進 pool，現在 pool 有 ${newPool.length} 筆。出塊之後才查得到。` };
    });

  const mine = () =>
    run(async () => {
      const result = await api.mineBlock();
      if (isBlock(result)) return { kind: "ok", text: `出塊成功！區塊 #${result.index}，裡面有 ${result.data.length} 筆交易。` };
      return { kind: "error", text: result.message ?? "沒有出塊" };
    });

  const search = () =>
    run(async () => {
      const found = await searchFiles(api, query);
      setResults(found);
      return found.length ? null : { kind: "info", text: `鏈上找不到名稱是「${query}」的檔案（還沒出塊的查不到）。` };
    });

  const nameOfKey = (key: string) => (key === unit.wallet.address ? "這個瀏覽器的單位金鑰" : short(key));

  return (
    <>
      <PageHeader
        title="FileChain：檔案存證"
        chapters={[{ label: "第 9 章：FileChain", doc: "docs/chapters/09-filechain.md" }]}
        code={["src/core/src/filechain.rs", "src/web/src/lib/filechain.ts"]}
      >
        <p>
          2018 年原本的專案：一個單位把圖片壓縮、用鏈的公鑰加密、用自己的私鑰簽章後上傳到節點；
          出塊之後，任何人都能用名稱查到檔案，並看到哪些單位驗證過、驗證等級是什麼。
        </p>
        <p>
          預設連到「瀏覽器裡的節點」，不用安裝任何東西。用 docker compose 起好 3 個節點之後，也可以切到真的節點。
        </p>
      </PageHeader>

      <div className="grid gap-4 lg:grid-cols-2">
        <Panel title="節點" aside={<button className="btn-secondary" onClick={refresh} aria-label="重新整理"><RefreshCw className="size-4" /></button>}>
          <div className="flex flex-wrap gap-2">
            <button className={mode === "browser" ? "btn-primary" : "btn-secondary"} onClick={() => setMode("browser")}>
              瀏覽器裡的節點
            </button>
            <button className={mode === "http" ? "btn-primary" : "btn-secondary"} onClick={() => setMode("http")}>
              <Server className="size-4" /> 真的節點
            </button>
          </div>
          {mode === "http" && (
            <div className="mt-3">
              <Field label="節點網址" hint="docker compose 的 3 個節點：http://localhost:3001、3002、3003。從哪個節點上傳、從哪個節點查詢都可以。">
                <input className="input" value={nodeUrl} onChange={(e) => setNodeUrl(e.target.value)} />
              </Field>
            </div>
          )}
          <dl className="mt-4 grid grid-cols-3 gap-2 text-center">
            <div className="rounded-lg bg-slate-50 p-2 dark:bg-slate-950">
              <dt className="label">區塊</dt>
              <dd className="text-xl font-bold tabular-nums">{blocks.length}</dd>
            </div>
            <div className="rounded-lg bg-slate-50 p-2 dark:bg-slate-950">
              <dt className="label">pool</dt>
              <dd className="text-xl font-bold tabular-nums">{pool.length}</dd>
            </div>
            <div className="rounded-lg bg-slate-50 p-2 dark:bg-slate-950">
              <dt className="label">連線節點</dt>
              <dd className="text-xl font-bold tabular-nums">{peers.length}</dd>
            </div>
          </dl>
          <button className="btn-primary mt-4 w-full" onClick={mine} disabled={busy}>
            <Pickaxe className="size-4" /> Mine Block（出塊）
          </button>
        </Panel>

        <Panel title="單位">
          <Field label="單位名稱" hint="簽章會涵蓋這個名稱。原本的程式把兩把公鑰寫死對應到 AMERICA、CHINA。">
            <input className="input" value={unit.name} onChange={(e) => setUnit({ ...unit, name: e.target.value })} />
          </Field>
          <p className="label mt-3">單位的公鑰（Ed25519）</p>
          <p className="hash text-slate-500">{unit.wallet.address}</p>
          <button className="btn-secondary mt-3" onClick={() => setUnit({ ...unit, wallet: core.generateWallet() })}>
            <KeyRound className="size-4" /> 換一把金鑰（當作另一個單位）
          </button>
          <p className="mt-2 text-xs text-slate-500">私鑰只存在這個分頁的記憶體裡，重新整理就換一把新的。</p>
        </Panel>
      </div>

      {message && (
        <div className="mt-4">
          <Notice kind={message.kind}>{message.text}</Notice>
        </div>
      )}

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <Panel title="上傳圖片">
          <label
            onDragOver={(e) => {
              e.preventDefault();
              setDragging(true);
            }}
            onDragLeave={() => setDragging(false)}
            onDrop={onDrop}
            className={`flex min-h-48 cursor-pointer flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed p-4 text-center transition ${
              dragging ? "border-indigo-500 bg-indigo-50 dark:bg-indigo-950" : "border-slate-300 dark:border-slate-700"
            }`}
          >
            {file ? (
              <img src={file.url} alt={`要上傳的圖片 ${file.name}`} className="max-h-40 rounded-lg" />
            ) : (
              <>
                <ImageUp className="size-8 text-slate-400" />
                <span className="text-sm text-slate-500">把圖片拖到這裡，或點一下選擇檔案</span>
              </>
            )}
            <input
              type="file"
              accept="image/*"
              className="sr-only"
              onChange={(e) => e.target.files?.[0] && choose(e.target.files[0].name, e.target.files[0])}
            />
          </label>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-xs">
            <span className="text-slate-500">或用範例圖片：</span>
            {SAMPLES.map((s) => (
              <button key={s.name} className="btn-secondary px-2 py-1 text-xs" onClick={async () => choose(s.name, await (await fetch(s.url)).blob())}>
                {s.name}
              </button>
            ))}
            {file && (
              <button className="btn-secondary px-2 py-1 text-xs" onClick={() => setFile(null)}>
                <Trash2 className="size-3.5" /> 移除
              </button>
            )}
          </div>
          <div className="mt-4 grid gap-3 sm:grid-cols-2">
            <Field label="Certification Owner Name（名稱）">
              <input className="input" value={owner} onChange={(e) => setOwner(e.target.value)} />
            </Field>
            <Field label="Certification Level（驗證等級）">
              <select className="input" value={level} onChange={(e) => setLevel(e.target.value as CerLevel)}>
                {CER_LEVELS.map((l) => (
                  <option key={l} value={l}>
                    {LEVEL_LABEL[l]}
                  </option>
                ))}
              </select>
            </Field>
          </div>
          <button className="btn-primary mt-4 w-full" onClick={upload} disabled={busy || !file || !owner.trim()}>
            <FileCheck2 className="size-4" /> 壓縮、加密、簽章，然後上傳
          </button>
        </Panel>

        <Panel title="搜尋">
          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void search();
            }}
          >
            <input className="input" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search name..." aria-label="要查的名稱" />
            <button className="btn-primary shrink-0" disabled={busy || !query.trim()}>
              <Search className="size-4" /> 查詢
            </button>
          </form>
          {results && results.length > 0 && (
            <div className="mt-4 grid gap-3 sm:grid-cols-2">
              {results.map((found) => (
                <FoundCard key={found.hit.fileHash} found={found} nameOfKey={nameOfKey} />
              ))}
            </div>
          )}
        </Panel>
      </div>

      <Panel title={`pool（${pool.length} 筆還沒上鏈）和鏈（${blocks.length} 個區塊）`} className="mt-4">
        <details>
          <summary className="cursor-pointer text-sm text-slate-600 dark:text-slate-300">看原始 JSON（GET /poolList、GET /blocks）</summary>
          <pre className="mt-2 max-h-96 overflow-auto rounded-lg bg-slate-900 p-3 text-xs text-slate-100">
            {JSON.stringify({ pool, blocks }, (key, value) => (key === "base64Data" && typeof value === "string" ? `${value.slice(0, 40)}…（${value.length} 字元）` : value), 2)}
          </pre>
        </details>
      </Panel>
    </>
  );
}
