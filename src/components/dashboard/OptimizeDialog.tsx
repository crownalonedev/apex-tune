import { useState } from "react";
import { Check, Circle, Loader2, X } from "lucide-react";
import { toast } from "sonner";
import { RestartBadge, RiskBadge, restartAction } from "@/components/badges";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { plural } from "@/lib/format";
import {
  api,
  onPresetProgress,
  toAppError,
  type AppError,
  type BatchReport,
  type BatchResult,
  type PresetItem,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/app";
import { useSafetyStore } from "@/stores/safety";
import { useTweaksStore } from "@/stores/tweaks";

type Phase =
  | { name: "loading" }
  | { name: "review" }
  | { name: "needsRestorePoint"; message: string }
  | { name: "applying" }
  | { name: "done"; report: BatchReport }
  | { name: "error"; error: AppError };

/** "Optimize for Gaming": preview exactly what changes, then apply with live progress. */
export function OptimizeDialog() {
  const [open, setOpen] = useState(false);
  const [phase, setPhase] = useState<Phase>({ name: "loading" });
  const [items, setItems] = useState<PresetItem[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [results, setResults] = useState<Map<string, BatchResult>>(new Map());
  const needRestart = useAppStore((s) => s.needRestart);

  const loadPreview = async () => {
    setPhase({ name: "loading" });
    setResults(new Map());
    try {
      const preview = await api.presetPreview();
      setItems(preview);
      setSelected(new Set(preview.map((i) => i.id)));
      setPhase({ name: "review" });
    } catch (error) {
      setPhase({ name: "error", error: toAppError(error) });
    }
  };

  const onOpenChange = (next: boolean) => {
    // Don't let a click outside abandon a batch halfway through.
    if (!next && phase.name === "applying") return;
    setOpen(next);
    if (next) void loadPreview();
  };

  const apply = async () => {
    setPhase({ name: "applying" });
    try {
      await api.createRestorePoint();
    } catch (error) {
      const e = toAppError(error);
      setPhase(e.kind === "restorePointRequired" ? { name: "needsRestorePoint", message: e.message } : { name: "error", error: e });
      return;
    }

    const unlisten = await onPresetProgress((result) =>
      setResults((prev) => new Map(prev).set(result.id, result)),
    );
    try {
      const ids = items.filter((i) => selected.has(i.id)).map((i) => i.id);
      const report = await api.applyPreset(ids);
      setResults(new Map(report.results.map((r) => [r.id, r])));
      setPhase({ name: "done", report });
      needRestart(report.restart);
      const failed = report.results.filter((r) => r.status === "failed").length;
      if (failed === 0) toast.success("Gaming preset applied");
      else toast.warning(`${plural(failed, "tweak")} failed and ${failed === 1 ? "was" : "were"} undone`);
    } catch (error) {
      setPhase({ name: "error", error: toAppError(error) });
    } finally {
      unlisten();
      void useTweaksStore.getState().load();
      void useSafetyStore.getState().load();
    }
  };

  const resolveRestorePoint = async (choice: "enable" | "skip") => {
    try {
      if (choice === "enable") await api.enableSystemProtection();
      else await api.skipRestorePoint();
      await apply();
    } catch (error) {
      setPhase({ name: "error", error: toAppError(error) });
    }
  };

  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const chosen = items.filter((i) => selected.has(i.id));

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button variant="primary" size="lg">
          Optimize for Gaming
        </Button>
      </DialogTrigger>
      <DialogContent>
        <div className="border-b border-line px-6 pt-5 pb-4 pr-12">
          <DialogTitle>Optimize for Gaming</DialogTitle>
          <DialogDescription>
            The Safe preset. Review exactly what changes; every change is backed up and can be reverted.
          </DialogDescription>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
          {phase.name === "loading" && (
            <div className="space-y-3" aria-busy="true">
              {Array.from({ length: 4 }, (_, i) => (
                <Skeleton key={i} className="h-14 w-full" />
              ))}
            </div>
          )}

          {phase.name === "error" && (
            <p role="alert" className="text-danger">
              {phase.error.message}
            </p>
          )}

          {phase.name === "needsRestorePoint" && (
            <div>
              <p className="text-moderate">{phase.message}</p>
              <p className="mt-2 text-muted">
                A restore point lets Windows roll back the whole system if something goes wrong. Each change is
                still backed up individually either way.
              </p>
              <div className="mt-4 flex flex-wrap gap-2">
                <Button variant="primary" onClick={() => void resolveRestorePoint("enable")}>
                  Turn on System Protection and continue
                </Button>
                <Button variant="ghost" onClick={() => void resolveRestorePoint("skip")}>
                  Continue without a restore point
                </Button>
              </div>
            </div>
          )}

          {phase.name === "review" &&
            (items.length === 0 ? (
              <p className="text-muted">Everything in the gaming preset is already applied.</p>
            ) : (
              <ul className="divide-y divide-line">
                {items.map((item) => (
                  <ReviewRow key={item.id} item={item} checked={selected.has(item.id)} onToggle={() => toggle(item.id)} />
                ))}
              </ul>
            ))}

          {(phase.name === "applying" || phase.name === "done") && (
            <ol className="space-y-2" aria-live="polite">
              {chosen.map((item, index) => {
                const result = results.get(item.id);
                const current = phase.name === "applying" && !result && index === results.size;
                return <ProgressRow key={item.id} name={item.name} result={result} current={current} />;
              })}
            </ol>
          )}
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-line px-6 py-4">
          <FooterNote phase={phase} hasItems={items.length > 0} />
          <div className="flex gap-2">
            {phase.name === "review" && items.length > 0 && (
              <Button variant="primary" onClick={() => void apply()} disabled={chosen.length === 0}>
                Apply {plural(chosen.length, "tweak")}
              </Button>
            )}
            {phase.name === "applying" && (
              <Button variant="primary" disabled>
                <Loader2 className="animate-spin" /> Applying…
              </Button>
            )}
            {(phase.name === "done" || phase.name === "error" || (phase.name === "review" && items.length === 0)) && (
              <Button onClick={() => setOpen(false)}>Close</Button>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function ReviewRow({ item, checked, onToggle }: { item: PresetItem; checked: boolean; onToggle: () => void }) {
  const inputId = `preset-${item.id}`;
  return (
    <li className="py-3">
      <div className="flex gap-3">
        <input
          id={inputId}
          type="checkbox"
          checked={checked}
          onChange={onToggle}
          className="mt-1 size-4 shrink-0 accent-[var(--signal)]"
        />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <label htmlFor={inputId} className="font-medium">
              {item.name}
            </label>
            <RiskBadge risk={item.risk} />
            <RestartBadge restart={item.restart} />
            {item.state.state === "partial" && <span className="text-[12px] text-faint">Partly applied</span>}
          </div>
          <p className="mt-0.5 text-[13px] text-muted">{item.description}</p>
          <details className="mt-1.5 text-[13px]">
            <summary className="cursor-pointer text-signal">Show {plural(item.changes.length, "change")}</summary>
            <ul className="selectable mt-1.5 space-y-1.5 border-l border-line pl-3">
              {item.changes.map((c) => (
                <li key={c.target}>
                  <span className="block break-all text-muted">{c.target}</span>
                  <span className="text-faint">{c.from}</span>
                  <span className="text-faint"> becomes </span>
                  <span>{c.to}</span>
                </li>
              ))}
            </ul>
          </details>
        </div>
      </div>
    </li>
  );
}

function ProgressRow({ name, result, current }: { name: string; result?: BatchResult; current: boolean }) {
  const icon = !result ? (
    current ? (
      <Loader2 className="size-4 animate-spin text-signal" aria-hidden="true" />
    ) : (
      <Circle className="size-4 text-faint" aria-hidden="true" />
    )
  ) : result.status === "failed" ? (
    <X className="size-4 text-danger" aria-hidden="true" />
  ) : (
    <Check className={cn("size-4", result.status === "applied" ? "text-signal" : "text-faint")} aria-hidden="true" />
  );
  const status = !result
    ? current
      ? "Applying"
      : "Waiting"
    : result.status === "applied"
      ? "Applied"
      : result.status === "alreadyApplied"
        ? "Already applied"
        : "Failed";

  return (
    <li className="flex gap-3">
      <span className="mt-0.5">{icon}</span>
      <div className="min-w-0 flex-1">
        <span className={cn(!result && !current && "text-muted")}>{name}</span>
        <span className="sr-only">: {status}</span>
        {result?.error && <p className="mt-0.5 text-[13px] text-danger">{result.error}</p>}
      </div>
    </li>
  );
}

function FooterNote({ phase, hasItems }: { phase: Phase; hasItems: boolean }) {
  if (phase.name === "done") {
    const { results, restart } = phase.report;
    const applied = results.filter((r) => r.status === "applied").length;
    const failed = results.filter((r) => r.status === "failed").length;
    return (
      <p className="text-[13px] text-muted">
        {plural(applied, "tweak")} applied{failed > 0 ? `, ${failed} failed and undone` : ""}.
        {restart !== "none" && ` ${restartAction[restart]} to finish.`}
      </p>
    );
  }
  if (phase.name === "review" && hasItems) {
    return <p className="text-[13px] text-muted">A restore point is created first if this session doesn't have one.</p>;
  }
  return <span />;
}
