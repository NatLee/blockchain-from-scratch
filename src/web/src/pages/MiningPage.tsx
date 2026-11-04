import { Pickaxe, Play, Square } from "lucide-react";
import { useRef, useState } from "react";
import { BarChart } from "../components/BarChart";
import { Field, Hash, PageHeader, Panel } from "../components/ui";
import { core, mineAsync } from "../lib/core";
import type { ChainParams, TextBlock } from "../lib/types";

const START_TIME = 1_700_000_000_000;

function MineOne() {
  const [data, setData] = useState("Alice 付給 Bob 10 元");
  const [difficulty, setDifficulty] = useState(16);
  const [progress, setProgress] = useState<{ attempts: number; block: TextBlock; ms: number } | null>(null);
  const [result, setResult] = useState<{ block: TextBlock; attempts: number; ms: number } | null>(null);
  const abort = useRef<AbortController | null>(null);

  const start = async () => {
    const controller = new AbortController();
    abort.current = controller;
    setResult(null);
    const genesis = core.genesisBlock();
    const params: ChainParams = { ...core.defaultParams(), initialDifficulty: difficulty };
    const template = core.nextBlock([genesis], params, Date.now(), data);
    const started = performance.now();
    const mined = await mineAsync(
      template,
      (attempts, block) => setProgress({ attempts, block, ms: performance.now() - started }),
      controller.signal,
    );
    abort.current = null;
    if (mined) setResult({ ...mined, ms: performance.now() - started });
  };

  const running = abort.current !== null;
  const rate = progress && progress.ms > 0 ? Math.round((progress.attempts / progress.ms) * 1000) : 0;

  return (
    <Panel title="挖一個區塊">
      <div className="grid gap-4 md:grid-cols-[1fr_16rem]">
        <Field label="區塊內容">
          <input className="input" value={data} onChange={(e) => setData(e.target.value)} />
        </Field>
        <Field label={`難度：${difficulty}（平均要試 ${core.blockWork(difficulty).toLocaleString()} 次）`}>
          <input
            type="range"
            min={0}
            max={24}
            value={difficulty}
            onChange={(e) => setDifficulty(Number(e.target.value))}
            className="w-full accent-indigo-600"
            disabled={running}
          />
        </Field>
      </div>
      <div className="mt-4 flex gap-2">
        <button className="btn-primary" onClick={start} disabled={running}>
          <Pickaxe className="size-4" /> 開始挖礦
        </button>
        <button className="btn-secondary" onClick={() => abort.current?.abort()} disabled={!running}>
          <Square className="size-4" /> 停止
        </button>
      </div>

      {progress && (
        <div className="mt-4 grid gap-3 sm:grid-cols-3">
          <div>
            <p className="label">試了幾次</p>
            <p className="text-2xl font-bold tabular-nums">{progress.attempts.toLocaleString()}</p>
          </div>
          <div>
            <p className="label">速度</p>
            <p className="text-2xl font-bold tabular-nums">{rate.toLocaleString()} 次/秒</p>
          </div>
          <div>
            <p className="label">花了</p>
            <p className="text-2xl font-bold tabular-nums">{(progress.ms / 1000).toFixed(2)} 秒</p>
          </div>
        </div>
      )}

      {result && (
        <div className="mt-4 space-y-1 rounded-lg bg-emerald-50 p-3 text-sm dark:bg-emerald-950">
          <p className="font-medium text-emerald-800 dark:text-emerald-200">
            挖到了！nonce = {result.block.nonce.toLocaleString()}，試了 {result.attempts.toLocaleString()} 次。
          </p>
          <p className="text-emerald-800 dark:text-emerald-200">
            hash 開頭有 {core.leadingZeroBits(result.block.hash)} 個 0 bit（要求至少 {difficulty} 個）：
          </p>
          <Hash value={result.block.hash} />
        </div>
      )}
    </Panel>
  );
}

interface SimBlock {
  index: number;
  difficulty: number;
  seconds: number;
}

/** 難度調整：用你設定的「算力」模擬出塊時間，看 Rust 的 `next_difficulty` 怎麼調。 */
function Retarget() {
  const [hashRate, setHashRate] = useState(20_000);
  const [target, setTarget] = useState(2);
  const [history, setHistory] = useState<SimBlock[]>([]);

  const run = () => {
    const params: ChainParams = {
      initialDifficulty: 8,
      minDifficulty: 4,
      maxDifficulty: 22,
      targetBlockTimeMs: target * 1000,
      adjustmentInterval: 5,
      blockReward: 50,
    };
    const blocks = [core.genesisBlock()];
    const out: SimBlock[] = [];
    let time = START_TIME;
    for (let i = 0; i < 40; i++) {
      const difficulty = core.nextDifficulty(blocks, params);
      // 每次嘗試成功的機率是 1/2^難度，所以要試幾次是幾何分布。用亂數抽一個次數，
      // 再用「次數 ÷ 算力」當作出塊時間，模擬一台每秒算 hashRate 次的電腦。
      const p = 1 / Number(core.blockWork(difficulty));
      const attempts = Math.max(1, Math.ceil(Math.log(1 - Math.random()) / Math.log(1 - p)));
      const ms = Math.max(1, Math.round((attempts / hashRate) * 1000));
      time += ms;
      // 難度由 Rust 的 next_difficulty 決定；這裡的區塊沒有真的挖礦，只用來記時間和難度。
      blocks.push(core.nextBlock(blocks, params, time, `block ${i + 1}`));
      out.push({ index: i + 1, difficulty, seconds: ms / 1000 });
    }
    setHistory(out);
  };

  return (
    <Panel title="難度自動調整">
      <p className="mb-4 text-sm text-slate-600 dark:text-slate-300">
        每 5 個區塊檢查一次：最近 5 個區塊出得比目標快一倍以上就把難度加 1，慢一倍以上就減 1。
        把算力調高，看難度怎麼跟上來，讓出塊時間回到目標附近。這裡的出塊時間用機率模擬（不用真的等），調整難度的是教材裡的 Rust 程式。
      </p>
      <div className="grid gap-4 md:grid-cols-3">
        <Field label={`模擬算力：每秒 ${hashRate.toLocaleString()} 次`}>
          <input type="range" min={2_000} max={200_000} step={2_000} value={hashRate} onChange={(e) => setHashRate(Number(e.target.value))} className="w-full accent-indigo-600" />
        </Field>
        <Field label={`目標出塊時間：${target} 秒`}>
          <input type="range" min={1} max={10} value={target} onChange={(e) => setTarget(Number(e.target.value))} className="w-full accent-indigo-600" />
        </Field>
        <div className="flex items-end">
          <button className="btn-primary w-full" onClick={run}>
            <Play className="size-4" /> 模擬 40 個區塊
          </button>
        </div>
      </div>
      {history.length > 0 && (
        <div className="mt-6 grid gap-6 lg:grid-cols-2">
          <BarChart
            title="每個區塊的出塊時間（秒）"
            unit="秒"
            bars={history.map((b) => ({ label: `#${b.index}`, value: b.seconds, detail: `難度 ${b.difficulty}` }))}
            reference={{ value: target, label: `目標 ${target} 秒` }}
          />
          <BarChart
            title="每個區塊的難度（開頭要幾個 0 bit）"
            unit="bit"
            bars={history.map((b) => ({ label: `#${b.index}`, value: b.difficulty, detail: `出塊 ${b.seconds} 秒` }))}
          />
        </div>
      )}
    </Panel>
  );
}

export default function MiningPage() {
  return (
    <>
      <PageHeader
        title="工作量證明：讓改歷史變得很貴"
        chapters={[{ label: "第 4 章：工作量證明和難度", doc: "docs/chapters/04-proof-of-work.md" }]}
        code={["src/core/src/pow.rs", "src/core/src/params.rs"]}
      >
        <p>
          挖礦就是一直換 nonce、一直算雜湊，直到 hash 開頭有夠多個 0。雜湊無法預測，所以沒有捷徑：
          難度每加 1，平均要試的次數就變成兩倍。
        </p>
      </PageHeader>
      <div className="space-y-4">
        <MineOne />
        <Retarget />
      </div>
    </>
  );
}
