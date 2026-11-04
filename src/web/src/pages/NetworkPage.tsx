import { Pause, Pickaxe, Play, RotateCcw, Scissors, Send, Shuffle, Unplug } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Field, Notice, PageHeader, Panel } from "../components/ui";
import { SimNetwork, core } from "../lib/core";
import { errorMessage, hashColor, short } from "../lib/format";
import type { SimSnapshot } from "../lib/types";

const NAMES = ["A", "B", "C", "D"];
const POSITIONS = [
  { x: 110, y: 70 },
  { x: 490, y: 70 },
  { x: 490, y: 250 },
  { x: 110, y: 250 },
];
/** 一開始接成一個環：A—B—C—D—A。對角線可以點一下接上。 */
const RING: [number, number][] = [
  [0, 1],
  [1, 2],
  [2, 3],
  [0, 3],
];
const ALL_PAIRS: [number, number][] = [...RING, [0, 2], [1, 3]];
/** 訊息延遲：故意設得很慢，才看得到訊息在路上跑。 */
const LATENCY_MS = 1_200;
const PARAMS = JSON.stringify({ ...core.defaultParams(), initialDifficulty: 12, minDifficulty: 12, maxDifficulty: 12 });

const MESSAGE_KINDS = [
  { prefix: "RESPONSE_BLOCKCHAIN", label: "區塊", className: "fill-indigo-500" },
  { prefix: "BROADCAST_TRANSACTION", label: "交易", className: "fill-amber-500" },
  { prefix: "QUERY", label: "詢問", className: "fill-slate-400" },
];

function kindOf(summary: string) {
  return MESSAGE_KINDS.find((k) => summary.startsWith(k.prefix)) ?? MESSAGE_KINDS[2];
}

function createNetwork() {
  const net = new SimNetwork(NAMES.length, PARAMS, LATENCY_MS, Date.now());
  for (const [a, b] of RING) net.connect(a, b);
  return net;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export default function NetworkPage() {
  const netRef = useRef<SimNetwork | null>(null);
  if (!netRef.current) netRef.current = createNetwork();
  const [snapshot, setSnapshot] = useState<SimSnapshot>(() => JSON.parse(netRef.current!.snapshot()));
  const [paused, setPaused] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [notice, setNotice] = useState<{ kind: "ok" | "error" | "info"; text: string } | null>(null);
  const [scripting, setScripting] = useState(false);
  const people = useMemo(() => ({ Bob: core.generateWallet().address, Carol: core.generateWallet().address }), []);
  const [payer, setPayer] = useState(0);
  const [via, setVia] = useState(0);
  const [recipient, setRecipient] = useState("Bob");
  const [amount, setAmount] = useState(10);

  const settings = useRef({ paused, speed });
  settings.current = { paused, speed };

  // 動畫：每一格畫面讓模擬時間往前走，再拍一張快照重畫。
  useEffect(() => {
    let frame = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const elapsed = now - last;
      last = now;
      if (!settings.current.paused && netRef.current) {
        netRef.current.advance(elapsed * settings.current.speed);
        setSnapshot(JSON.parse(netRef.current.snapshot()));
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, []);

  const net = () => netRef.current!;
  const miners = snapshot.nodes.map((n) => n.address);
  const nameOf = (address?: string | null) => {
    if (!address) return "—";
    const i = miners.indexOf(address);
    if (i >= 0) return `礦工 ${NAMES[i]}`;
    return (Object.keys(people) as (keyof typeof people)[]).find((p) => people[p] === address) ?? short(address);
  };
  const isLinked = (a: number, b: number) => snapshot.links.some(([x, y]) => x === Math.min(a, b) && y === Math.max(a, b));

  const toggle = (a: number, b: number) => {
    if (isLinked(a, b)) net().disconnect(a, b);
    else net().connect(a, b);
  };
  const partition = () => {
    for (const [a, b] of ALL_PAIRS) {
      const sameSide = (a < 2) === (b < 2);
      if (!sameSide) net().disconnect(a, b);
    }
    setNotice({ kind: "info", text: "網路斷成兩邊：{A, B} 和 {C, D}。兩邊各自出塊，就會分岔。" });
  };
  const reconnect = () => {
    for (const [a, b] of RING) net().connect(a, b);
    setNotice({ kind: "info", text: "重新接上線。節點會互相問最新的區塊，累積工作量比較多的那條鏈勝出。" });
  };
  const reset = () => {
    netRef.current?.free();
    netRef.current = createNetwork();
    setSnapshot(JSON.parse(netRef.current.snapshot()));
    setNotice(null);
  };
  const mine = (i: number) => {
    const block = JSON.parse(net().mine(i));
    setNotice({ kind: "ok", text: `礦工 ${NAMES[i]} 挖到區塊 #${block.index}，正在廣播給相鄰的節點。` });
  };
  const send = () => {
    const to = recipient in people ? people[recipient as keyof typeof people] : miners[Number(recipient)];
    try {
      net().transfer(via, payer, to, amount, Date.now());
      setNotice({ kind: "ok", text: `礦工 ${NAMES[payer]} 付 ${amount} 元給 ${nameOf(to)}，交易送到節點 ${NAMES[via]}。` });
    } catch (e) {
      setNotice({ kind: "error", text: `節點 ${NAMES[via]} 拒絕這筆交易：${errorMessage(e)}` });
    }
  };

  /** 第 8 章的雙重支付：礦工 A 在兩邊各花一次同一筆錢。 */
  const doubleSpend = async () => {
    setScripting(true);
    setPaused(false);
    reset();
    const step = async (text: string, wait: number, action: () => void) => {
      action();
      setNotice({ kind: "info", text });
      await sleep(wait / settings.current.speed);
    };
    await step("1. 礦工 A 挖到一個區塊，拿到 50 元，等大家都收到。", 4_500, () => net().mine(0));
    await step("2. 網路斷成兩邊：{A, B} 和 {C, D}。", 800, partition);
    await step("3. 左邊：A 付 50 元給 Bob，送到節點 A。", 1_800, () => net().transfer(0, 0, people.Bob, 50, Date.now()));
    await step("4. 右邊：A 又把同一筆 50 元付給 Carol，送到節點 C。", 1_800, () => net().transfer(2, 0, people.Carol, 50, Date.now() + 1));
    await step("5. 左邊的 A 挖到區塊，把付給 Bob 的交易放進去。", 2_000, () => net().mine(0));
    await step("6. 右邊的 C 挖到區塊，把付給 Carol 的交易放進去。", 2_000, () => net().mine(2));
    await step("7. 右邊的 D 又挖了一個區塊：右邊的累積工作量比較多。", 2_500, () => net().mine(3));
    await step("8. 重新接上線……", 6_000, reconnect);
    setNotice({
      kind: "ok",
      text: "大家都換到右邊的鏈。Bob 收到的 50 元跟著左邊的區塊一起消失了：付給 Bob 的交易被放回 mempool，但錢已經付給 Carol，所以被丟掉。",
    });
    setScripting(false);
  };

  const recentLog = snapshot.log.slice(-10).reverse();
  const start = snapshot.log[0]?.time ?? snapshot.now;

  return (
    <>
      <PageHeader
        title="P2P 網路和分岔"
        chapters={[
          { label: "第 7 章：P2P 節點和廣播", doc: "docs/chapters/07-p2p.md" },
          { label: "第 8 章：分岔和共識", doc: "docs/chapters/08-consensus.md" },
        ]}
        code={["src/core/src/node.rs", "src/core/src/consensus.rs", "src/core/src/sim.rs"]}
      >
        <p>
          四個節點，每個都在你的瀏覽器裡跑教材的 Rust 程式。訊息故意延遲 {LATENCY_MS / 1000} 秒，看得到區塊和交易在網路上傳。
          點一下兩個節點之間的線可以拔掉或接上；斷成兩邊後各自挖礦就會分岔，重新接上後，大家會換到累積工作量最多的鏈。
        </p>
      </PageHeader>

      <Panel>
        <div className="mb-3 flex flex-wrap gap-2">
          <button className="btn-secondary" onClick={() => setPaused((p) => !p)}>
            {paused ? <Play className="size-4" /> : <Pause className="size-4" />} {paused ? "繼續" : "暫停"}
          </button>
          <select className="input w-auto" value={speed} onChange={(e) => setSpeed(Number(e.target.value))} aria-label="播放速度">
            <option value={0.5}>0.5 倍速</option>
            <option value={1}>1 倍速</option>
            <option value={2}>2 倍速</option>
          </select>
          <button className="btn-secondary" onClick={partition} disabled={scripting}>
            <Scissors className="size-4" /> 分成兩邊
          </button>
          <button className="btn-secondary" onClick={reconnect} disabled={scripting}>
            <Unplug className="size-4" /> 重新接上
          </button>
          <button className="btn-primary" onClick={doubleSpend} disabled={scripting}>
            <Shuffle className="size-4" /> 雙重支付示範
          </button>
          <button className="btn-secondary ml-auto" onClick={reset} disabled={scripting}>
            <RotateCcw className="size-4" /> 重新開始
          </button>
        </div>

        <div className="grid gap-4 lg:grid-cols-[1fr_20rem]">
          <svg viewBox="0 0 600 320" className="w-full rounded-xl bg-slate-50 dark:bg-slate-950" role="img" aria-label="四個節點的網路圖">
            {ALL_PAIRS.map(([a, b]) => {
              const linked = isLinked(a, b);
              return (
                <g key={`${a}-${b}`} className="cursor-pointer" onClick={() => toggle(a, b)}>
                  <title>{linked ? `拔掉 ${NAMES[a]}—${NAMES[b]}` : `接上 ${NAMES[a]}—${NAMES[b]}`}</title>
                  <line x1={POSITIONS[a].x} y1={POSITIONS[a].y} x2={POSITIONS[b].x} y2={POSITIONS[b].y} stroke="transparent" strokeWidth={18} />
                  <line
                    x1={POSITIONS[a].x}
                    y1={POSITIONS[a].y}
                    x2={POSITIONS[b].x}
                    y2={POSITIONS[b].y}
                    strokeWidth={linked ? 3 : 2}
                    strokeDasharray={linked ? undefined : "6 6"}
                    className={linked ? "stroke-slate-400 dark:stroke-slate-500" : "stroke-slate-300 dark:stroke-slate-700"}
                  />
                </g>
              );
            })}
            {snapshot.inFlight.map((m, i) => {
              const progress = Math.min(1, Math.max(0, 1 - (m.deliverAt - snapshot.now) / LATENCY_MS));
              const from = POSITIONS[m.from];
              const to = POSITIONS[m.to];
              return (
                <circle
                  key={i}
                  cx={from.x + (to.x - from.x) * progress}
                  cy={from.y + (to.y - from.y) * progress}
                  r={7}
                  className={`${kindOf(m.summary).className} stroke-white dark:stroke-slate-950`}
                  strokeWidth={2}
                >
                  <title>{m.summary}</title>
                </circle>
              );
            })}
            {snapshot.nodes.map((node, i) => (
              <g key={i}>
                <circle cx={POSITIONS[i].x} cy={POSITIONS[i].y} r={34} fill={hashColor(node.tipHash)} className="stroke-white dark:stroke-slate-900" strokeWidth={4} />
                <text x={POSITIONS[i].x} y={POSITIONS[i].y - 2} textAnchor="middle" className="fill-white text-lg font-bold">
                  {NAMES[i]}
                </text>
                <text x={POSITIONS[i].x} y={POSITIONS[i].y + 16} textAnchor="middle" className="fill-white text-[11px]">
                  高度 {node.height}
                </text>
              </g>
            ))}
          </svg>

          <div className="space-y-3">
            <div className="flex flex-wrap gap-3 text-xs text-slate-600 dark:text-slate-300">
              {MESSAGE_KINDS.map((k) => (
                <span key={k.label} className="inline-flex items-center gap-1">
                  <svg className="size-3" viewBox="0 0 10 10" aria-hidden>
                    <circle cx={5} cy={5} r={5} className={k.className} />
                  </svg>
                  {k.label}
                </span>
              ))}
              <span>節點顏色＝最新區塊的 hash</span>
            </div>
            {notice && <Notice kind={notice.kind}>{notice.text}</Notice>}
            <div>
              <p className="label mb-1">發生了什麼（送了 {snapshot.delivered} 則訊息）</p>
              <ol className="max-h-56 space-y-1 overflow-y-auto text-xs">
                {recentLog.map((entry, i) => (
                  <li key={i} className="flex gap-2">
                    <span className="w-12 shrink-0 text-right text-slate-400 tabular-nums">{((entry.time - start) / 1000).toFixed(1)}s</span>
                    <span className="font-semibold">{NAMES[entry.node]}</span>
                    <span className="text-slate-600 dark:text-slate-300">{entry.text}</span>
                  </li>
                ))}
              </ol>
            </div>
          </div>
        </div>
      </Panel>

      <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {snapshot.nodes.map((node, i) => (
          <Panel key={i} title={`節點 ${NAMES[i]}`} aside={<button className="btn-primary" onClick={() => mine(i)} disabled={scripting}><Pickaxe className="size-4" /> 挖礦</button>}>
            <p className="label mb-1">鏈（顏色＝區塊的 hash）</p>
            <div className="mb-3 flex flex-wrap gap-1">
              {node.blocks.slice(-12).map((block) => (
                <span
                  key={block.hash}
                  title={`#${block.index} ${short(block.hash)}，礦工 ${nameOf(block.miner)}`}
                  className="flex size-7 items-center justify-center rounded-md text-[10px] font-semibold text-white"
                  style={{ background: hashColor(block.hash) }}
                >
                  {block.index}
                </span>
              ))}
            </div>
            <dl className="grid grid-cols-2 gap-1 text-xs">
              <dt className="text-slate-500">累積工作量</dt>
              <dd className="text-right tabular-nums">{BigInt(node.totalWork).toLocaleString()}</dd>
              <dt className="text-slate-500">mempool</dt>
              <dd className="text-right">{node.mempool.length} 筆</dd>
              {[...miners, people.Bob, people.Carol].map((address) =>
                node.balances[address] ? (
                  <div key={address} className="contents">
                    <dt className="text-slate-500">{nameOf(address)}</dt>
                    <dd className="text-right tabular-nums">{node.balances[address]} 元</dd>
                  </div>
                ) : null,
              )}
            </dl>
          </Panel>
        ))}
      </div>

      <Panel title="轉帳" className="mt-4">
        <div className="flex flex-wrap items-end gap-2">
          <Field label="付款人">
            <select className="input" value={payer} onChange={(e) => setPayer(Number(e.target.value))}>
              {NAMES.map((n, i) => (
                <option key={n} value={i}>
                  礦工 {n}
                </option>
              ))}
            </select>
          </Field>
          <Field label="收款人">
            <select className="input" value={recipient} onChange={(e) => setRecipient(e.target.value)}>
              {NAMES.map((n, i) => (
                <option key={n} value={String(i)}>
                  礦工 {n}
                </option>
              ))}
              {Object.keys(people).map((p) => (
                <option key={p}>{p}</option>
              ))}
            </select>
          </Field>
          <Field label="金額">
            <input type="number" min={1} className="input w-24" value={amount} onChange={(e) => setAmount(Number(e.target.value))} />
          </Field>
          <Field label="送到哪個節點">
            <select className="input" value={via} onChange={(e) => setVia(Number(e.target.value))}>
              {NAMES.map((n, i) => (
                <option key={n} value={i}>
                  節點 {n}
                </option>
              ))}
            </select>
          </Field>
          <button className="btn-primary" onClick={send} disabled={scripting}>
            <Send className="size-4" /> 送出
          </button>
        </div>
      </Panel>
    </>
  );
}
