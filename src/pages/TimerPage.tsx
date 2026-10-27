import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { toast } from "sonner";
import { TweakCard } from "@/components/TweakCard";
import { PageHeader } from "@/components/layout/PageHeader";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { routeById } from "@/lib/routes";
import { api, onBenchProgress, toAppError, type BenchReport, type TimerStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useTweaksStore } from "@/stores/tweaks";

/** Resolutions offered, in 100 ns units. */
const CHOICES = [5_000, 5_040, 5_070, 7_500, 10_000];
const ms = (units: number) => (units / 10_000).toFixed(3);

export function TimerPage() {
  const route = routeById("timer");
  const [status, setStatus] = useState<TimerStatus | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const loadTweaks = useTweaksStore((s) => s.load);
  const globalTweak = useTweaksStore((s) => s.overview?.tweaks.find((t) => t.id === "global_timer_requests"));

  useEffect(() => {
    void loadTweaks();
    let alive = true;
    const refresh = () =>
      api
        .timerStatus()
        .then((s) => alive && setStatus(s))
        .catch(() => {});
    void refresh();
    const timer = setInterval(refresh, 2000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [loadTweaks]);

  const units = selected ?? status?.units ?? 5_000;

  const act = async (action: () => Promise<TimerStatus>, done: string) => {
    setBusy(true);
    try {
      setStatus(await action());
      toast.success(done);
    } catch (error) {
      toast.error(toAppError(error).message, { duration: 12_000 });
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <PageHeader title={route.label} summary={route.summary} />

      <div className="mt-8 grid gap-x-10 gap-y-10 xl:grid-cols-2">
        <section aria-labelledby="timer-now">
          <h2 id="timer-now" className="text-[19px] font-semibold">
            Right now
          </h2>
          {status?.resolution ? (
            <dl className="mt-3 grid grid-cols-3 gap-px overflow-hidden rounded-lg border border-line bg-line">
              <Readout label="Current" value={status.resolution.current} highlight />
              <Readout label="Finest possible" value={status.resolution.finest} />
              <Readout label="Windows default" value={status.resolution.coarsest} />
            </dl>
          ) : status ? (
            <p className="mt-3 text-muted">Windows didn't report its timer resolution.</p>
          ) : (
            <Skeleton className="mt-3 h-20 w-full" />
          )}
          <p className="mt-3 max-w-[68ch] text-[13px] text-muted">
            A finer timer makes Sleep() wake closer to on time, which steadies frame pacing in games and frame
            limiters that pace with it. Many games already request 1 ms themselves; the benefit is usually small
            but measurable.
          </p>
        </section>

        <section aria-labelledby="timer-hold">
          <h2 id="timer-hold" className="text-[19px] font-semibold">
            Hold a resolution
          </h2>
          <div role="radiogroup" aria-label="Timer resolution" className="mt-3 flex flex-wrap gap-2">
            {CHOICES.map((c) => (
              <button
                key={c}
                type="button"
                role="radio"
                aria-checked={units === c}
                onClick={() => setSelected(c)}
                className={cn(
                  "h-9 rounded-md border px-3 text-sm tabular-nums transition-colors",
                  units === c
                    ? "border-signal bg-signal-soft text-signal"
                    : "border-line-strong text-muted hover:text-text",
                )}
              >
                {ms(c)} ms
              </button>
            ))}
          </div>
          <div className="mt-4 flex flex-wrap items-center gap-3">
            {status?.helperRunning ? (
              <>
                <span className="text-signal">Holding {ms(status.units)} ms from the tray.</span>
                {units !== status.units && (
                  <Button size="sm" variant="primary" disabled={busy} onClick={() => void act(() => api.timerStart(units), `Now holding ${ms(units)} ms`)}>
                    Switch to {ms(units)} ms
                  </Button>
                )}
                <Button size="sm" variant="ghost" disabled={busy} onClick={() => void act(api.timerStop, "Timer helper stopped")}>
                  Stop
                </Button>
              </>
            ) : (
              <Button
                variant="primary"
                disabled={busy || !status?.helperAvailable}
                onClick={() => void act(() => api.timerStart(units), `Holding ${ms(units)} ms`)}
              >
                Start holding {ms(units)} ms
              </Button>
            )}
          </div>
          {status && !status.helperAvailable && (
            <p className="mt-2 text-[13px] text-moderate">
              apex-timer.exe isn't next to Apex Tune. In development, build it with{" "}
              <code className="selectable">cargo build -p apex-timer</code>.
            </p>
          )}
          <label className="mt-4 flex items-center gap-3 text-sm">
            <Switch
              checked={status?.autostart ?? false}
              disabled={busy || !status?.helperAvailable}
              onCheckedChange={(v) =>
                void act(() => api.timerSetAutostart(v), v ? "Timer helper starts with Windows" : "Removed from startup")
              }
            />
            Start the helper when I sign in
          </label>
        </section>
      </div>

      <Benchmark onUse={(u) => setSelected(u)} />

      {globalTweak && (
        <section aria-labelledby="timer-global" className="mt-10">
          <h2 id="timer-global" className="mb-3 text-[19px] font-semibold">
            Make it apply to games
          </h2>
          <TweakCard tweak={globalTweak} />
        </section>
      )}
    </>
  );
}

function Readout({ label, value, highlight }: { label: string; value: number; highlight?: boolean }) {
  return (
    <div className="bg-panel px-4 py-3">
      <dt className="text-[13px] text-muted">{label}</dt>
      <dd className={cn("text-[24px] font-semibold tabular-nums [font-stretch:80%]", highlight && "text-signal")}>
        {value.toFixed(3)} <span className="text-[14px] font-normal text-muted">ms</span>
      </dd>
    </div>
  );
}

function Benchmark({ onUse }: { onUse: (units: number) => void }) {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState(0);
  const [report, setReport] = useState<BenchReport | null>(null);

  const run = async () => {
    setRunning(true);
    setProgress(0);
    const unlisten = await onBenchProgress((done, total) => setProgress(total ? done / total : 0));
    try {
      setReport(await api.timerBenchmark());
    } catch (error) {
      toast.error(toAppError(error).message);
    } finally {
      unlisten();
      setRunning(false);
    }
  };

  return (
    <section aria-labelledby="timer-bench" className="mt-10">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 id="timer-bench" className="text-[19px] font-semibold">
            Benchmark this PC
          </h2>
          <p className="mt-1 max-w-[68ch] text-muted">
            Times 300 Sleep(1) calls at each resolution and picks the one that wakes closest to 1 ms. Ties go to the
            coarser value, which uses less power.
          </p>
        </div>
        <Button onClick={() => void run()} disabled={running}>
          {running ? "Measuring…" : report ? "Run again" : "Run benchmark (about 10 s)"}
        </Button>
      </div>

      <AnimatePresence>
        {running && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="mt-4 h-1.5 overflow-hidden rounded-full bg-panel-raised"
            role="progressbar"
            aria-valuenow={Math.round(progress * 100)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label="Benchmark progress"
          >
            <motion.div className="h-full bg-signal" animate={{ width: `${progress * 100}%` }} transition={{ ease: "easeOut" }} />
          </motion.div>
        )}
      </AnimatePresence>

      {report && (
        <table className="mt-4 w-full border-y border-line text-left tabular-nums">
          <thead>
            <tr className="text-[13px] text-muted">
              <th scope="col" className="py-2 pr-4 font-normal">Request</th>
              <th scope="col" className="py-2 pr-4 font-normal">Average Sleep(1)</th>
              <th scope="col" className="py-2 pr-4 font-normal">99% of sleeps under</th>
              <th scope="col" className="py-2 pr-4 font-normal">Worst</th>
              <th scope="col" className="py-2 font-normal"><span className="sr-only">Action</span></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {report.rows.map((row) => {
              const best = row.requested !== null && row.requested === report.recommended;
              return (
                <tr key={row.requested ?? "none"} className={cn(best && "bg-signal-soft")}>
                  <th scope="row" className="py-2.5 pr-4 font-normal">
                    {row.requested === null ? "No request (Windows default)" : `${ms(row.requested)} ms`}
                    {best && <Badge tone="signal" className="ml-2">Best for this PC</Badge>}
                  </th>
                  <td className="py-2.5 pr-4">{row.meanMs.toFixed(3)} ms</td>
                  <td className="py-2.5 pr-4">{row.p99Ms.toFixed(3)} ms</td>
                  <td className="py-2.5 pr-4">{row.maxMs.toFixed(3)} ms</td>
                  <td className="py-2.5 text-right">
                    {row.requested !== null && (
                      <Button size="sm" variant={best ? "primary" : "ghost"} onClick={() => onUse(row.requested!)}>
                        Use
                      </Button>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </section>
  );
}
