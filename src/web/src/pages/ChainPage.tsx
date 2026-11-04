import { Hammer, Pickaxe, Plus, RotateCcw } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Hash, Notice, PageHeader, Panel, StatusBadge } from "../components/ui";
import { core, mineAsync } from "../lib/core";
import { short } from "../lib/format";
import type { BlockStatus, ChainParams, TextBlock } from "../lib/types";

const START_TIME = 1_700_000_000_000;
const SAMPLE_DATA = ["Alice 付給 Bob 10 元", "Bob 付給 Carol 5 元", "Carol 付給 Dave 1 元"];
const DIFFICULTIES = [
  { value: 0, label: "0（第 3 章：沒有工作量證明）" },
  { value: 12, label: "12（第 4 章：平均要試 4096 次）" },
  { value: 16, label: "16（平均要試 65536 次）" },
];

/** 難度固定不變的參數（跟 Rust 的 `ChainParams::fixed_difficulty` 一樣）。 */
function fixedParams(difficulty: number): ChainParams {
  return { ...core.defaultParams(), initialDifficulty: difficulty, minDifficulty: difficulty, maxDifficulty: difficulty };
}

async function buildChain(params: ChainParams): Promise<TextBlock[]> {
  const blocks = [core.genesisBlock()];
  for (const [i, data] of SAMPLE_DATA.entries()) {
    const mined = await mineAsync(core.nextBlock(blocks, params, START_TIME + i * 60_000, data), () => {});
    blocks.push(mined!.block);
  }
  return blocks;
}

function statusBorder(status?: BlockStatus) {
  if (!status) return "border-slate-200 dark:border-slate-800";
  return {
    valid: "border-emerald-400 dark:border-emerald-700",
    invalid: "border-rose-500 dark:border-rose-600",
    buildsOnInvalid: "border-amber-400 dark:border-amber-600",
  }[status.status];
}

function BlockCard(props: {
  block: TextBlock;
  status?: BlockStatus;
  busy: boolean;
  onData: (data: string) => void;
  onRehash: () => void;
  onRemine: () => void;
}) {
  const { block, status } = props;
  const isGenesis = block.index === 0;
  return (
    <article className={`w-72 shrink-0 rounded-2xl border-2 bg-white p-4 shadow-sm dark:bg-slate-900 ${statusBorder(status)}`}>
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h3 className="shrink-0 font-semibold text-slate-900 dark:text-white">{isGenesis ? "創世區塊 #0" : `區塊 #${block.index}`}</h3>
        {status && <StatusBadge status={status} />}
      </div>

      <div className="space-y-2 rounded-lg bg-slate-50 p-3 text-xs dark:bg-slate-950">
        <p className="label">標頭（header）</p>
        <dl className="grid grid-cols-[5.5rem_1fr] gap-x-2 gap-y-1">
          <dt className="text-slate-500">previousHash</dt>
          <dd className="hash" title={block.previousHash}>{short(block.previousHash, 10)}</dd>
          <dt className="text-slate-500">timestamp</dt>
          <dd className="mono">{block.timestamp}</dd>
          <dt className="text-slate-500">dataHash</dt>
          <dd className="hash" title={block.dataHash}>{short(block.dataHash, 10)}</dd>
          <dt className="text-slate-500">difficulty</dt>
          <dd className="mono">{block.difficulty}</dd>
          <dt className="text-slate-500">nonce</dt>
          <dd className="mono">{block.nonce}</dd>
        </dl>
      </div>

      <div className="mt-3 space-y-1">
        <p className="label">內容（data）</p>
        <textarea
          className="input min-h-16 text-sm"
          value={block.data}
          disabled={isGenesis}
          onChange={(e) => props.onData(e.target.value)}
          aria-label={`區塊 ${block.index} 的內容`}
        />
      </div>

      <div className="mt-3 space-y-1">
        <p className="label">hash</p>
        <Hash value={block.hash} />
      </div>

      {!isGenesis && (
        <div className="mt-3 flex gap-2">
          <button className="btn-secondary flex-1" onClick={props.onRehash} disabled={props.busy} title="重新算 dataHash 和 hash，不挖礦">
            <Hammer className="size-4" /> 重算雜湊
          </button>
          <button className="btn-primary flex-1" onClick={props.onRemine} disabled={props.busy} title="接到前一個區塊的新 hash，再挖礦">
            <Pickaxe className="size-4" /> 重新挖礦
          </button>
        </div>
      )}
    </article>
  );
}

export default function ChainPage() {
  const [difficulty, setDifficulty] = useState(0);
  const params = useMemo(() => fixedParams(difficulty), [difficulty]);
  const [blocks, setBlocks] = useState<TextBlock[]>([]);
  const [busy, setBusy] = useState(false);
  const [newData, setNewData] = useState("Dave 付給 Erin 2 元");
  const [lastWork, setLastWork] = useState<string | null>(null);

  const reset = async (p: ChainParams) => {
    setBusy(true);
    setBlocks(await buildChain(p));
    setLastWork(null);
    setBusy(false);
  };

  useEffect(() => {
    void reset(params);
  }, [params]);

  const statuses = useMemo(() => (blocks.length ? core.checkTextChain(blocks, params) : []), [blocks, params]);
  const allValid = statuses.every((s) => s.status === "valid");

  const update = (index: number, block: TextBlock) => setBlocks((bs) => bs.map((b, i) => (i === index ? block : b)));

  const remine = async (index: number) => {
    setBusy(true);
    // 攻擊者的做法：接到前一個區塊現在的 hash，nonce 從 0 開始重新挖。
    const relinked = core.rehashBlock({ ...blocks[index], previousHash: blocks[index - 1].hash, nonce: 0 });
    const mined = await mineAsync(relinked, () => {});
    if (mined) {
      update(index, mined.block);
      setLastWork(`區塊 #${index} 重新挖礦：試了 ${mined.attempts.toLocaleString()} 次雜湊`);
    }
    setBusy(false);
  };

  const append = async () => {
    setBusy(true);
    const template = core.nextBlock(blocks, params, START_TIME + blocks.length * 60_000, newData);
    const mined = await mineAsync(template, () => {});
    if (mined) setBlocks((bs) => [...bs, mined.block]);
    setBusy(false);
  };

  return (
    <>
      <PageHeader
        title="區塊和鏈：改一個，全部對不上"
        chapters={[
          { label: "第 2 章：區塊", doc: "docs/chapters/02-block.md" },
          { label: "第 3 章：鏈和竄改偵測", doc: "docs/chapters/03-chain.md" },
        ]}
        code={["src/core/src/block.rs", "src/core/src/chain.rs"]}
      >
        <p>
          每個區塊的 hash 是標頭的 SHA-256，標頭裡有內容的雜湊（dataHash）和前一個區塊的 hash（previousHash）。
          改任何一個區塊的內容看看：那個區塊先壞掉，後面的區塊也跟著不能相信。
        </p>
        <p>
          接著按「重算雜湊」或「重新挖礦」修好它，看看要修到哪裡才會全部變綠。難度 0 時重算一下就好；
          難度調高之後，每個區塊都得重新挖礦，這就是工作量證明保護歷史的方式。
        </p>
      </PageHeader>

      <Panel className="mb-4">
        <div className="flex flex-wrap items-end gap-3">
          <label className="space-y-1">
            <span className="label">難度</span>
            <select className="input" value={difficulty} onChange={(e) => setDifficulty(Number(e.target.value))} disabled={busy}>
              {DIFFICULTIES.map((d) => (
                <option key={d.value} value={d.value}>
                  {d.label}
                </option>
              ))}
            </select>
          </label>
          <button className="btn-secondary" onClick={() => reset(params)} disabled={busy}>
            <RotateCcw className="size-4" /> 重新開始
          </button>
          <div className="ml-auto flex min-w-64 flex-1 gap-2">
            <input className="input" value={newData} onChange={(e) => setNewData(e.target.value)} aria-label="新區塊的內容" />
            <button className="btn-primary shrink-0" onClick={append} disabled={busy || !allValid} title={allValid ? "" : "鏈有問題時不能接新區塊"}>
              <Plus className="size-4" /> 接一個區塊
            </button>
          </div>
        </div>
      </Panel>

      <div className="mb-4 space-y-2">
        {allValid ? (
          <Notice kind="ok">整條鏈都正常：每個區塊的 hash 都對得上，也都接得上前一個區塊。</Notice>
        ) : (
          <Notice kind="error">鏈被改過了。從第一個紅色的區塊開始，後面的區塊都不能相信。</Notice>
        )}
        {lastWork && <Notice kind="info">{lastWork}</Notice>}
      </div>

      <div className="-mx-4 overflow-x-auto px-4 pb-4">
        <div className="flex items-stretch gap-3">
          {blocks.map((block, i) => (
            <div key={i} className="flex items-stretch gap-3">
              {i > 0 && <span className="self-center text-2xl text-slate-400" aria-hidden>→</span>}
              <BlockCard
                block={block}
                status={statuses[i]}
                busy={busy}
                onData={(data) => update(i, { ...block, data })}
                onRehash={() => update(i, core.rehashBlock({ ...block, previousHash: blocks[i - 1].hash }))}
                onRemine={() => remine(i)}
              />
            </div>
          ))}
        </div>
      </div>

      {blocks[1] && (
        <Panel title="區塊的 hash 是怎麼算的" className="mt-2">
          <p className="mb-2 text-sm text-slate-600 dark:text-slate-300">
            把標頭的欄位用 <code>|</code> 接起來，再算 SHA-256（<code>Header::hash_input</code>）。區塊 #1 的標頭：
          </p>
          <pre className="overflow-x-auto rounded-lg bg-slate-900 p-3 text-xs text-slate-100">{core.hashInput(blocks[1])}</pre>
        </Panel>
      )}
    </>
  );
}
