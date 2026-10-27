import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ChevronRight } from "lucide-react";
import { toast } from "sonner";
import { SafetyPanel } from "@/components/SafetyPanel";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { formatWmiDate, plural } from "@/lib/format";
import { api, toAppError, type EntryView, type SessionView } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useSafetyStore } from "@/stores/safety";
import { useTweaksStore } from "@/stores/tweaks";

const when = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });
const time = new Intl.DateTimeFormat(undefined, { timeStyle: "short" });

function statusBadge(e: EntryView) {
  if (e.status === "rolledBack") return <Badge tone="moderate">Failed, undone</Badge>;
  if (e.status === "rollbackFailed") return <Badge tone="advanced">Needs attention</Badge>;
  if (e.status === "inProgress") return <Badge tone="moderate">Interrupted</Badge>;
  if (e.action === "revert") return <Badge>Revert</Badge>;
  if (e.reverted) return <Badge>Reverted</Badge>;
  return <Badge tone="signal">In effect</Badge>;
}

export function HistoryPanel() {
  const [sessions, setSessions] = useState<SessionView[] | null>(null);
  const [busy, setBusy] = useState(false);
  const activeChanges = useSafetyStore((s) => s.status?.activeChanges);

  const load = () =>
    api
      .backupsSessions()
      .then(setSessions)
      .catch((e) => toast.error(toAppError(e).message));

  // Reload when Revert Everything (in the safety panel) changes things.
  useEffect(() => {
    void load();
  }, [activeChanges]);

  const revert = async (e: EntryView) => {
    setBusy(true);
    try {
      await api.backupsRevertEntry(e.session, e.entry);
      toast.success(`Reverted: ${e.name}`);
    } catch (err) {
      toast.error(toAppError(err).message, { duration: 12_000 });
    } finally {
      setBusy(false);
      await load();
      void useSafetyStore.getState().load();
      void useTweaksStore.getState().load();
    }
  };

  return (
    <div className="mt-5 space-y-8">
      <div className="max-w-3xl">
        <SafetyPanel />
      </div>
      <section>
        <h2 className="text-[19px] font-semibold">What changed</h2>
        <p className="mt-1 max-w-[72ch] text-muted">Every change by session, with each setting's value before and after. Revert any one on its own.</p>
        {!sessions ? (
          <Skeleton className="mt-3 h-32 w-full" />
        ) : sessions.length === 0 ? (
          <p className="mt-3 text-muted">Nothing has been changed yet.</p>
        ) : (
          <div className="mt-3 space-y-6">
            {sessions.map((s) => (
              <section key={s.id} aria-label={`Session ${when.format(new Date(s.startedAt))}`}>
                <h3 className="flex flex-wrap items-baseline gap-3 border-b border-line pb-1.5">
                  <span className="font-semibold">{when.format(new Date(s.startedAt))}</span>
                  <span className="text-[12px] text-faint">
                    {s.restorePoint
                      ? `Restore point ${formatWmiDate(s.restorePoint.created)}`
                      : s.restorePointSkipped
                        ? "No restore point (skipped)"
                        : "No restore point"}
                  </span>
                </h3>
                <ul className="divide-y divide-line">
                  {s.entries.map((e) => (
                    <Entry key={e.entry} entry={e} busy={busy} onRevert={() => void revert(e)} />
                  ))}
                </ul>
              </section>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function Entry({ entry, busy, onRevert }: { entry: EntryView; busy: boolean; onRevert: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <li className="py-2">
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={() => setOpen(!open)}
          aria-expanded={open}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          <ChevronRight className={cn("size-4 shrink-0 text-faint transition-transform", open && "rotate-90")} aria-hidden="true" />
          <span className="truncate">{entry.name}</span>
          <span className="shrink-0 text-[12px] text-faint">
            {time.format(new Date(entry.at))}, {plural(entry.changes.length, "setting")}
          </span>
        </button>
        {statusBadge(entry)}
        {entry.revertible && (
          <Button size="sm" variant="ghost" disabled={busy} onClick={onRevert}>
            Revert
          </Button>
        )}
      </div>
      {entry.error && <p className="mt-1 pl-6 text-[12px] text-moderate">{entry.error}</p>}
      <AnimatePresence initial={false}>
        {open && (
          <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
            <table className="selectable mt-2 ml-6 w-[calc(100%-1.5rem)] text-left text-[12px]">
              <thead className="text-faint">
                <tr>
                  <th className="py-1 pr-3 font-normal">Setting</th>
                  <th className="py-1 pr-3 font-normal">Before</th>
                  <th className="py-1 font-normal">After</th>
                </tr>
              </thead>
              <tbody>
                {entry.changes.map((c, i) => (
                  <tr key={i} className="align-top">
                    <td className="py-1 pr-3 break-all text-muted">{c.target}</td>
                    <td className="py-1 pr-3">{c.before}</td>
                    <td className="py-1">{c.after}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </motion.div>
        )}
      </AnimatePresence>
    </li>
  );
}
