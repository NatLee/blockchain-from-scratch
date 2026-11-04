import { useState } from "react";

export interface Bar {
  label: string;
  value: number;
  /** 滑鼠移上去時顯示的說明。 */
  detail: string;
}

/**
 * 一個系列的長條圖（SVG）。一條可選的參考線（例如目標出塊時間），滑鼠移到長條上會顯示數值。
 * 只有一個系列，所以不放圖例，由標題說明在畫什麼。
 */
export function BarChart(props: {
  title: string;
  bars: Bar[];
  unit: string;
  reference?: { value: number; label: string };
  height?: number;
}) {
  const { bars, reference } = props;
  const [hover, setHover] = useState<number | null>(null);
  const height = props.height ?? 160;
  const width = 640;
  const pad = { top: 12, right: 12, bottom: 22, left: 44 };
  const plotW = width - pad.left - pad.right;
  const plotH = height - pad.top - pad.bottom;
  const max = Math.max(1, ...bars.map((b) => b.value), reference?.value ?? 0) * 1.1;
  const y = (v: number) => pad.top + plotH - (v / max) * plotH;
  const slot = bars.length ? plotW / bars.length : plotW;
  const barW = Math.max(2, slot - 2); // 長條之間留 2px 的空隙
  const ticks = [0, max / 2, max].map((v) => Math.round(v));

  return (
    <figure className="space-y-2">
      <figcaption className="text-sm font-medium text-slate-700 dark:text-slate-200">{props.title}</figcaption>
      <div className="relative">
        <svg viewBox={`0 0 ${width} ${height}`} className="w-full" role="img" aria-label={props.title}>
          {ticks.map((t) => (
            <g key={t}>
              <line x1={pad.left} x2={width - pad.right} y1={y(t)} y2={y(t)} className="stroke-slate-200 dark:stroke-slate-800" />
              <text x={pad.left - 6} y={y(t) + 4} textAnchor="end" className="fill-slate-400 text-[10px]">
                {t.toLocaleString()}
              </text>
            </g>
          ))}
          {bars.map((bar, i) => {
            const top = y(bar.value);
            const h = Math.max(1, pad.top + plotH - top);
            const x = pad.left + i * slot + 1;
            return (
              <g key={i} onMouseEnter={() => setHover(i)} onMouseLeave={() => setHover(null)}>
                {/* 透明的大範圍，比長條本身好指 */}
                <rect x={pad.left + i * slot} y={pad.top} width={slot} height={plotH} fill="transparent" />
                <path
                  d={`M${x},${top + h} v${-(h - Math.min(4, h))} q0,${-Math.min(4, h)} ${Math.min(4, barW / 2)},${-Math.min(4, h)} h${barW - 2 * Math.min(4, barW / 2)} q${Math.min(4, barW / 2)},0 ${Math.min(4, barW / 2)},${Math.min(4, h)} v${h - Math.min(4, h)} z`}
                  className={hover === i ? "fill-indigo-400" : "fill-indigo-500 dark:fill-indigo-400"}
                />
              </g>
            );
          })}
          {reference && (
            <g>
              <line
                x1={pad.left}
                x2={width - pad.right}
                y1={y(reference.value)}
                y2={y(reference.value)}
                strokeDasharray="4 3"
                strokeWidth={2}
                className="stroke-slate-500 dark:stroke-slate-300"
              />
              <text x={width - pad.right} y={y(reference.value) - 4} textAnchor="end" className="fill-slate-600 text-[10px] dark:fill-slate-300">
                {reference.label}
              </text>
            </g>
          )}
          {bars.length > 0 && (
            <>
              <text x={pad.left} y={height - 6} className="fill-slate-400 text-[10px]">
                {bars[0].label}
              </text>
              <text x={width - pad.right} y={height - 6} textAnchor="end" className="fill-slate-400 text-[10px]">
                {bars[bars.length - 1].label}
              </text>
            </>
          )}
        </svg>
        {hover !== null && bars[hover] && (
          <div
            className="pointer-events-none absolute top-0 rounded-md bg-slate-900 px-2 py-1 text-xs text-white shadow"
            style={{ left: `${((pad.left + (hover + 0.5) * slot) / width) * 100}%`, transform: "translateX(-50%)" }}
          >
            {bars[hover].label}：{bars[hover].value.toLocaleString()} {props.unit}
            <div className="text-slate-300">{bars[hover].detail}</div>
          </div>
        )}
      </div>
    </figure>
  );
}
