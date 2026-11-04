import { BookOpen, CheckCircle2, CircleAlert, Code2, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import { docUrl } from "../config";
import type { BlockStatus } from "../lib/types";

/** 每個頁面的標題區：標題、說明、對應的章節和程式。 */
export function PageHeader(props: {
  title: string;
  chapters: { label: string; doc: string }[];
  code: string[];
  children: ReactNode;
}) {
  return (
    <header className="mb-6 space-y-3">
      <h1 className="text-2xl font-bold tracking-tight text-slate-900 sm:text-3xl dark:text-white">{props.title}</h1>
      <div className="max-w-3xl space-y-2 text-slate-600 dark:text-slate-300">{props.children}</div>
      <div className="flex flex-wrap gap-2 text-xs">
        {props.chapters.map((c) => (
          <a
            key={c.doc}
            href={docUrl(c.doc)}
            className="inline-flex items-center gap-1 rounded-full bg-indigo-50 px-2.5 py-1 font-medium text-indigo-700 hover:bg-indigo-100 dark:bg-indigo-950 dark:text-indigo-300"
          >
            <BookOpen className="size-3.5" aria-hidden />
            {c.label}
          </a>
        ))}
        {props.code.map((path) => (
          <a
            key={path}
            href={docUrl(path)}
            className="inline-flex items-center gap-1 rounded-full bg-slate-100 px-2.5 py-1 font-mono text-slate-600 hover:bg-slate-200 dark:bg-slate-800 dark:text-slate-300"
          >
            <Code2 className="size-3.5" aria-hidden />
            {path}
          </a>
        ))}
      </div>
    </header>
  );
}

export function Panel(props: { title?: string; aside?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`panel ${props.className ?? ""}`}>
      {(props.title || props.aside) && (
        <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
          {props.title && <h2 className="text-lg font-semibold text-slate-900 dark:text-white">{props.title}</h2>}
          {props.aside}
        </div>
      )}
      {props.children}
    </section>
  );
}

/** 雜湊：開頭的 0 用顏色標出來（挖礦就是在找開頭有很多 0 的雜湊）。 */
export function Hash({ value, className = "" }: { value: string; className?: string }) {
  const zeros = value.match(/^0*/)?.[0].length ?? 0;
  return (
    <span className={`hash ${className}`}>
      <span className="font-bold text-emerald-600 dark:text-emerald-400">{value.slice(0, zeros)}</span>
      {value.slice(zeros)}
    </span>
  );
}

const STATUS_STYLE = {
  valid: {
    icon: CheckCircle2,
    text: "正常",
    className: "text-emerald-700 bg-emerald-50 dark:text-emerald-300 dark:bg-emerald-950",
  },
  invalid: {
    icon: CircleAlert,
    text: "不合法",
    className: "text-rose-700 bg-rose-50 dark:text-rose-300 dark:bg-rose-950",
  },
  buildsOnInvalid: {
    icon: TriangleAlert,
    text: "接在有問題的區塊後面",
    className: "text-amber-700 bg-amber-50 dark:text-amber-300 dark:bg-amber-950",
  },
} as const;

/** 區塊狀態：圖示 + 文字，不只靠顏色。 */
export function StatusBadge({ status }: { status: BlockStatus }) {
  const style = STATUS_STYLE[status.status];
  const Icon = style.icon;
  return (
    <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${style.className}`}>
      <Icon className="size-3.5" aria-hidden />
      {status.status === "invalid" ? status.reason : style.text}
    </span>
  );
}

export function Field(props: { label: string; children: ReactNode; hint?: string }) {
  return (
    <label className="block space-y-1">
      <span className="label block">{props.label}</span>
      {props.children}
      {props.hint && <span className="block text-xs text-slate-500 dark:text-slate-400">{props.hint}</span>}
    </label>
  );
}

/** 錯誤或成功的小提示。 */
export function Notice({ kind, children }: { kind: "ok" | "error" | "info"; children: ReactNode }) {
  const style = {
    ok: "border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-200",
    error: "border-rose-200 bg-rose-50 text-rose-800 dark:border-rose-900 dark:bg-rose-950 dark:text-rose-200",
    info: "border-sky-200 bg-sky-50 text-sky-800 dark:border-sky-900 dark:bg-sky-950 dark:text-sky-200",
  }[kind];
  return (
    <div role={kind === "error" ? "alert" : "status"} className={`rounded-lg border px-3 py-2 text-sm ${style}`}>
      {children}
    </div>
  );
}
