import { useMemo, useState } from "react";
import { Field, Hash, PageHeader, Panel } from "../components/ui";
import { core } from "../lib/core";
import { hexToBits } from "../lib/format";

/** 256 個 bit 排成 16×16 的格子，兩個雜湊不一樣的 bit 標出來。 */
function BitGrid({ a, b }: { a: string; b: string }) {
  const bitsA = hexToBits(a);
  const bitsB = hexToBits(b);
  return (
    <div className="grid max-w-md grid-cols-16 gap-0.5" role="img" aria-label="兩個雜湊逐位元比較，翻轉的位元標成紅色">
      {[...bitsA].map((bit, i) => {
        const flipped = bit !== bitsB[i];
        return (
          <div
            key={i}
            title={`第 ${i + 1} 個 bit：${bit} → ${bitsB[i]}`}
            className={`aspect-square rounded-[3px] ${
              flipped ? "bg-rose-500" : bit === "1" ? "bg-slate-400 dark:bg-slate-500" : "bg-slate-200 dark:bg-slate-800"
            }`}
          />
        );
      })}
    </div>
  );
}

export default function HashPage() {
  const [a, setA] = useState("hello");
  const [b, setB] = useState("hellp");
  const hashA = useMemo(() => core.sha256Hex(a), [a]);
  const hashB = useMemo(() => core.sha256Hex(b), [b]);
  const diff = core.bitDifference(hashA, hashB);

  return (
    <>
      <PageHeader
        title="雜湊：資料的指紋"
        chapters={[{ label: "第 1 章：雜湊", doc: "docs/chapters/01-hash.md" }]}
        code={["src/core/src/hash.rs"]}
      >
        <p>
          SHA-256 把任何長度的資料變成 256 bit（64 個十六進位字元）。同樣的輸入永遠得到同樣的輸出；
          輸入只改一個字，輸出就有大約一半的 bit 會翻轉，這叫<strong>雪崩效應</strong>。
        </p>
      </PageHeader>

      <div className="grid gap-4 lg:grid-cols-2">
        <Panel title="輸入 A">
          <Field label={`文字（${new TextEncoder().encode(a).length} bytes）`}>
            <textarea className="input min-h-20 font-mono" value={a} onChange={(e) => setA(e.target.value)} />
          </Field>
          <p className="mt-3 label">SHA-256</p>
          <Hash value={hashA} />
        </Panel>
        <Panel title="輸入 B">
          <Field label={`文字（${new TextEncoder().encode(b).length} bytes）`}>
            <textarea className="input min-h-20 font-mono" value={b} onChange={(e) => setB(e.target.value)} />
          </Field>
          <p className="mt-3 label">SHA-256</p>
          <Hash value={hashB} />
        </Panel>
      </div>

      <Panel title="逐位元比較" className="mt-4">
        <div className="grid gap-6 md:grid-cols-[minmax(0,28rem)_1fr]">
          <BitGrid a={hashA} b={hashB} />
          <div className="space-y-3">
            <div>
              <p className="label">翻轉的 bit</p>
              <p className="text-4xl font-bold text-slate-900 tabular-nums dark:text-white">
                {diff}
                <span className="text-lg font-normal text-slate-500"> / 256</span>
              </p>
              <p className="text-sm text-slate-500 dark:text-slate-400">{((diff / 256) * 100).toFixed(1)}%，理論平均是 50%</p>
            </div>
            <ul className="space-y-1 text-sm text-slate-600 dark:text-slate-300">
              <li>
                <span className="mr-1 inline-block size-3 rounded-sm bg-rose-500 align-middle" /> 兩邊不一樣的 bit
              </li>
              <li>
                <span className="mr-1 inline-block size-3 rounded-sm bg-slate-400 align-middle" /> 一樣，都是 1
              </li>
              <li>
                <span className="mr-1 inline-block size-3 rounded-sm bg-slate-200 align-middle dark:bg-slate-800" /> 一樣，都是 0
              </li>
            </ul>
            <p className="text-sm text-slate-600 dark:text-slate-300">
              {a === b ? "兩邊一模一樣，雜湊也一模一樣。" : "試試看只改一個字母、加一個空白，或把大寫改小寫。"}
            </p>
          </div>
        </div>
      </Panel>
    </>
  );
}
