import { useEffect, useId } from "react";
import { Area, AreaChart, ResponsiveContainer, Tooltip, YAxis } from "recharts";
import { formatBytes } from "@/lib/format";
import type { StatsTick } from "@/lib/tauri";
import { useStatsStore } from "@/stores/stats";

interface Point {
  at: number;
  value: number | null;
}

/** Live CPU, GPU, memory and process count, sampled only while visible. */
export function LiveStats() {
  const history = useStatsStore((s) => s.history);
  const watch = useStatsStore((s) => s.watch);

  useEffect(() => watch(), [watch]);

  const latest = history.at(-1);
  const series = (pick: (t: StatsTick) => number | null): Point[] =>
    history.map((t) => ({ at: t.at, value: pick(t) }));

  const ramPercent = (t: StatsTick) => (t.ramTotalBytes ? (100 * t.ramUsedBytes) / t.ramTotalBytes : 0);

  return (
    <section aria-labelledby="live-heading">
      <h2 id="live-heading" className="text-[19px] font-semibold">
        Right now
      </h2>
      <div className="mt-3 grid grid-cols-1 gap-px overflow-hidden rounded-lg border border-line bg-line sm:grid-cols-2">
        <Tile
          label="CPU"
          value={latest ? `${Math.round(latest.cpuPercent)}%` : undefined}
          data={series((t) => t.cpuPercent)}
          format={(v) => `${Math.round(v)}%`}
        />
        <Tile
          label="GPU"
          value={latest ? (latest.gpuPercent === null ? "Not reported" : `${Math.round(latest.gpuPercent)}%`) : undefined}
          data={series((t) => t.gpuPercent)}
          format={(v) => `${Math.round(v)}%`}
        />
        <Tile
          label="Memory"
          value={latest ? `${formatBytes(latest.ramUsedBytes)} of ${formatBytes(latest.ramTotalBytes)}` : undefined}
          data={series(ramPercent)}
          format={(v) => `${Math.round(v)}% used`}
        />
        <Tile
          label="Processes"
          value={latest ? String(latest.processes) : undefined}
          data={series((t) => t.processes)}
          format={(v) => `${v} processes`}
          max="auto"
        />
      </div>
      <p className="mt-2 text-[13px] text-faint">
        GPU temperature arrives with the NVIDIA stage. CPU temperature isn't shown: Windows only exposes it through
        a kernel driver, and loading one could trip anti-cheat.
      </p>
    </section>
  );
}

interface TileProps {
  label: string;
  value: string | undefined;
  data: Point[];
  format: (v: number) => string;
  /** Y range; percentages default to 0–100. */
  max?: number | "auto";
}

function Tile({ label, value, data, format, max = 100 }: TileProps) {
  const gradient = useId();
  const hasData = data.some((d) => d.value !== null);

  return (
    <div className="bg-panel px-4 pt-3 pb-1">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-[13px] text-muted">{label}</span>
        <span className="text-[17px] font-semibold tabular-nums [font-stretch:85%]">
          {value ?? <span className="text-faint">…</span>}
        </span>
      </div>
      <div className="h-12" aria-hidden="true">
        {hasData && (
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={data} margin={{ top: 4, right: 0, bottom: 2, left: 0 }}>
              <defs>
                <linearGradient id={gradient} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="var(--chart)" stopOpacity={0.28} />
                  <stop offset="100%" stopColor="var(--chart)" stopOpacity={0} />
                </linearGradient>
              </defs>
              <YAxis hide domain={max === "auto" ? ["dataMin - 5", "dataMax + 5"] : [0, max]} />
              <Tooltip
                content={(props) => <SparkTooltip active={props.active} payload={props.payload} format={format} />}
                cursor={{ stroke: "var(--line-strong)", strokeWidth: 1 }}
                isAnimationActive={false}
              />
              <Area
                dataKey="value"
                type="monotone"
                stroke="var(--chart)"
                strokeWidth={2}
                fill={`url(#${gradient})`}
                isAnimationActive={false}
                connectNulls={false}
                dot={false}
                activeDot={{ r: 4, stroke: "var(--panel)", strokeWidth: 2, fill: "var(--chart)" }}
              />
            </AreaChart>
          </ResponsiveContainer>
        )}
      </div>
    </div>
  );
}

interface SparkTooltipProps {
  active?: boolean;
  payload?: ReadonlyArray<{ payload?: unknown }>;
  format: (v: number) => string;
}

function SparkTooltip({ active, payload, format }: SparkTooltipProps) {
  const point = payload?.[0]?.payload as Point | undefined;
  if (!active || !point || point.value === null) return null;
  const secondsAgo = Math.max(0, Math.round((Date.now() - point.at) / 1000));
  return (
    <div className="rounded-md border border-line-strong bg-panel-raised px-2 py-1 text-[12px] shadow-lg">
      <span className="font-medium text-text">{format(point.value)}</span>
      <span className="ml-2 text-muted">{secondsAgo === 0 ? "now" : `${secondsAgo}s ago`}</span>
    </div>
  );
}
