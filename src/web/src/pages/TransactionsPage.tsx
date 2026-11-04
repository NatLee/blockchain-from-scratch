import { Coins, PenLine, Pickaxe, RotateCcw, Send, ShieldCheck } from "lucide-react";
import { useMemo, useState } from "react";
import { Field, Notice, PageHeader, Panel } from "../components/ui";
import { SimNetwork, core } from "../lib/core";
import { errorMessage, short } from "../lib/format";
import type { SimSnapshot, Transaction, VerifyResult, Wallet } from "../lib/types";

type Person = "Alice" | "Bob" | "Mallory";

/** 第 5 章：簽一筆轉帳、改它、驗證它。 */
function SignaturePlayground() {
  const wallets = useMemo<Record<Person, Wallet>>(
    () => ({ Alice: core.generateWallet(), Bob: core.generateWallet(), Mallory: core.generateWallet() }),
    [],
  );
  const nameOf = (address: string) =>
    (Object.keys(wallets) as Person[]).find((p) => wallets[p].address === address) ?? short(address);
  const [amount, setAmount] = useState(10);
  const [text, setText] = useState("");
  const [result, setResult] = useState<VerifyResult | null>(null);

  const sign = () => {
    const tx = core.signTransfer(wallets.Alice.secret, wallets.Bob.address, amount, Date.now());
    setText(JSON.stringify(tx, null, 2));
    setResult(null);
  };

  const verify = (json = text) => {
    try {
      setResult(core.verifyTransaction(JSON.parse(json)));
    } catch (e) {
      setResult({ ok: false, error: errorMessage(e), id: "" });
    }
  };

  /** 改交易的某個欄位，再驗證一次。 */
  const tamper = (change: (tx: Extract<Transaction, { type: "transfer" }>) => void) => {
    const tx = JSON.parse(text);
    change(tx);
    const json = JSON.stringify(tx, null, 2);
    setText(json);
    verify(json);
  };

  const parsed = useMemo(() => {
    try {
      return JSON.parse(text) as Extract<Transaction, { type: "transfer" }>;
    } catch {
      return null;
    }
  }, [text]);

  return (
    <Panel title="簽章：證明是你本人，而且內容沒被改">
      <div className="mb-4 grid gap-2 text-xs sm:grid-cols-3">
        {(Object.keys(wallets) as Person[]).map((p) => (
          <div key={p} className="rounded-lg bg-slate-50 p-2 dark:bg-slate-950">
            <p className="font-semibold">{p} 的地址（公鑰）</p>
            <p className="hash text-slate-500">{wallets[p].address}</p>
          </div>
        ))}
      </div>
      <div className="flex flex-wrap items-end gap-2">
        <Field label="Alice 要轉給 Bob 的金額">
          <input type="number" min={1} className="input w-32" value={amount} onChange={(e) => setAmount(Number(e.target.value))} />
        </Field>
        <button className="btn-primary" onClick={sign}>
          <PenLine className="size-4" /> 用 Alice 的私鑰簽章
        </button>
      </div>

      {text && (
        <div className="mt-4 grid gap-4 lg:grid-cols-[1fr_18rem]">
          <Field label="交易（可以直接改裡面的 JSON）">
            <textarea className="input min-h-64 font-mono text-xs" value={text} onChange={(e) => setText(e.target.value)} />
          </Field>
          <div className="space-y-2">
            <button className="btn-primary w-full" onClick={() => verify()}>
              <ShieldCheck className="size-4" /> 驗證簽章
            </button>
            <p className="label pt-2">試試看當 Mallory</p>
            <button className="btn-secondary w-full" onClick={() => tamper((tx) => (tx.body.amount = 1000))}>
              把金額改成 1000
            </button>
            <button className="btn-secondary w-full" onClick={() => tamper((tx) => (tx.body.to = wallets.Mallory.address))}>
              把收款人改成 Mallory
            </button>
            <button
              className="btn-secondary w-full"
              onClick={() => {
                // Mallory 用自己的私鑰簽，卻把付款人寫成 Alice。
                const forged = core.signTransfer(wallets.Mallory.secret, wallets.Mallory.address, 1000, Date.now());
                const json = JSON.stringify({ ...forged, publicKey: wallets.Alice.address }, null, 2);
                setText(json);
                verify(json);
              }}
            >
              自己簽，假裝是 Alice
            </button>
            {parsed?.body && (
              <p className="text-xs text-slate-500">
                付款人 {nameOf(parsed.publicKey)} → 收款人 {nameOf(parsed.body.to)}，{parsed.body.amount} 元
              </p>
            )}
            {result &&
              (result.ok ? (
                <Notice kind="ok">簽章正確。交易編號 {short(result.id)}</Notice>
              ) : (
                <Notice kind="error">驗證失敗：{result.error}</Notice>
              ))}
          </div>
        </div>
      )}
    </Panel>
  );
}

/** 第 6 章：一個節點，看 mempool、出塊和餘額。 */
function MempoolPlayground() {
  const people = useMemo(() => ({ Bob: core.generateWallet().address, Carol: core.generateWallet().address }), []);
  const [net, setNet] = useState(() => new SimNetwork(1, JSON.stringify({ ...core.defaultParams(), initialDifficulty: 14, minDifficulty: 14 }), 0, Date.now()));
  const [snapshot, setSnapshot] = useState<SimSnapshot>(() => JSON.parse(net.snapshot()));
  const [to, setTo] = useState<keyof typeof people>("Bob");
  const [amount, setAmount] = useState(30);
  const [message, setMessage] = useState<{ kind: "ok" | "error"; text: string } | null>(null);
  const node = snapshot.nodes[0];
  const miner = node.address;
  const nameOf = (address?: string) =>
    address === miner ? "你（礦工）" : (Object.keys(people) as (keyof typeof people)[]).find((p) => people[p] === address) ?? short(address);

  const refresh = () => setSnapshot(JSON.parse(net.snapshot()));

  const send = (amounts: number[]) => {
    const results: string[] = [];
    let failed = false;
    for (const [i, a] of amounts.entries()) {
      try {
        net.transfer(0, 0, people[to], a, Date.now() + i);
        results.push(`${a} 元：放進 mempool`);
      } catch (e) {
        failed = true;
        results.push(`${a} 元：被拒絕（${errorMessage(e)}）`);
      }
    }
    setMessage({ kind: failed ? "error" : "ok", text: results.join("；") });
    refresh();
  };

  const mine = () => {
    const block = JSON.parse(net.mine(0));
    net.advance(1_000);
    setMessage({ kind: "ok", text: `挖到區塊 #${block.index}，裡面有 ${block.data.length} 筆交易（第一筆是給你的出塊獎勵）` });
    refresh();
  };

  const reset = () => {
    net.free();
    const fresh = new SimNetwork(1, JSON.stringify({ ...core.defaultParams(), initialDifficulty: 14, minDifficulty: 14 }), 0, Date.now());
    setNet(fresh);
    setSnapshot(JSON.parse(fresh.snapshot()));
    setMessage(null);
  };

  return (
    <Panel
      title="mempool 和出塊"
      aside={
        <button className="btn-secondary" onClick={reset}>
          <RotateCcw className="size-4" /> 重新開始
        </button>
      }
    >
      <p className="mb-4 text-sm text-slate-600 dark:text-slate-300">
        你是礦工。先挖礦拿出塊獎勵，再轉帳給 Bob 或 Carol。交易會先放進 mempool 排隊，等你下次挖礦才上鏈。
        試試看「同時送兩筆 30 元」：mempool 會把排隊中的交易也算進去，第二筆因為錢不夠被拒絕。
      </p>
      <div className="flex flex-wrap items-end gap-2">
        <button className="btn-primary" onClick={mine}>
          <Pickaxe className="size-4" /> 挖礦
        </button>
        <Field label="收款人">
          <select className="input" value={to} onChange={(e) => setTo(e.target.value as keyof typeof people)}>
            {Object.keys(people).map((p) => (
              <option key={p}>{p}</option>
            ))}
          </select>
        </Field>
        <Field label="金額">
          <input type="number" min={1} className="input w-28" value={amount} onChange={(e) => setAmount(Number(e.target.value))} />
        </Field>
        <button className="btn-secondary" onClick={() => send([amount])}>
          <Send className="size-4" /> 轉帳
        </button>
        <button className="btn-secondary" onClick={() => send([30, 30])}>
          同時送兩筆 30 元
        </button>
      </div>
      {message && (
        <div className="mt-3">
          <Notice kind={message.kind}>{message.text}</Notice>
        </div>
      )}

      <div className="mt-4 grid gap-4 lg:grid-cols-3">
        <div>
          <p className="label mb-2">餘額（從整條鏈重算）</p>
          <ul className="space-y-1 text-sm">
            {[miner, people.Bob, people.Carol].map((address) => (
              <li key={address} className="flex justify-between rounded-lg bg-slate-50 px-3 py-2 dark:bg-slate-950">
                <span>{nameOf(address)}</span>
                <span className="inline-flex items-center gap-1 font-semibold tabular-nums">
                  <Coins className="size-4 text-amber-500" aria-hidden /> {node.balances[address] ?? 0}
                </span>
              </li>
            ))}
          </ul>
        </div>
        <div>
          <p className="label mb-2">mempool（{node.mempool.length} 筆排隊中）</p>
          <ul className="space-y-1 text-sm">
            {node.mempool.length === 0 && <li className="text-slate-500">沒有排隊中的交易</li>}
            {node.mempool.map((tx) => (
              <li key={tx.id} className="rounded-lg border border-dashed border-slate-300 px-3 py-2 dark:border-slate-700">
                {nameOf(tx.from)} → {nameOf(tx.to)}：{tx.amount} 元
              </li>
            ))}
          </ul>
        </div>
        <div>
          <p className="label mb-2">鏈（高度 {node.height}）</p>
          <ol className="max-h-72 space-y-1 overflow-y-auto text-sm">
            {[...node.blocks].reverse().map((block) => (
              <li key={block.hash} className="rounded-lg bg-slate-50 px-3 py-2 dark:bg-slate-950">
                <p className="font-medium">
                  #{block.index} <span className="hash text-slate-500">{short(block.hash)}</span>
                </p>
                {block.transactions.map((tx) => (
                  <p key={tx.id} className="text-xs text-slate-600 dark:text-slate-300">
                    {tx.type === "coinbase" ? `出塊獎勵 ${tx.amount} 元 → ${nameOf(tx.to)}` : `${nameOf(tx.from)} → ${nameOf(tx.to)}：${tx.amount} 元`}
                  </p>
                ))}
              </li>
            ))}
          </ol>
        </div>
      </div>
    </Panel>
  );
}

export default function TransactionsPage() {
  return (
    <>
      <PageHeader
        title="簽章、交易和出塊"
        chapters={[
          { label: "第 5 章：金鑰、簽章和交易", doc: "docs/chapters/05-signatures.md" },
          { label: "第 6 章：mempool 和出塊", doc: "docs/chapters/06-mempool.md" },
        ]}
        code={["src/core/src/wallet.rs", "src/core/src/transaction.rs", "src/core/src/mempool.rs", "src/core/src/ledger.rs"]}
      >
        <p>
          區塊鏈上沒有帳號密碼：「這筆錢是我的」靠的是 Ed25519 數位簽章。私鑰簽、公鑰驗，內容改一個字簽章就對不上。
          這頁的金鑰只存在這個分頁的記憶體裡，重新整理就不見了。
        </p>
      </PageHeader>
      <div className="space-y-4">
        <SignaturePlayground />
        <MempoolPlayground />
      </div>
    </>
  );
}
