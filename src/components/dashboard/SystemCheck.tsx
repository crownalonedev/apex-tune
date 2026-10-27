import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CircleAlert, CircleCheck, Info, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { api, toAppError, type Finding } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useSafetyStore } from "@/stores/safety";

const icon = {
  good: <CircleCheck className="size-4 text-safe" aria-label="Good" />,
  info: <Info className="size-4 text-signal" aria-label="Note" />,
  warn: <CircleAlert className="size-4 text-moderate" aria-label="Needs attention" />,
};

export function SystemCheck() {
  const [findings, setFindings] = useState<Finding[] | null>(null);
  const [busy, setBusy] = useState(false);

  const run = () =>
    api
      .insightsRun()
      .then(setFindings)
      .catch((e) => toast.error(toAppError(e).message));

  useEffect(() => {
    void run();
  }, []);

  const fix = async (f: Finding) => {
    if (!f.fix) return;
    setBusy(true);
    try {
      await api.insightsFix(f.fix);
      toast.success(`Switched to ${f.fix.hz} Hz`, { description: "Revert it from Profiles & backups if you change your mind." });
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 12_000 });
    } finally {
      setBusy(false);
      await run();
      void useSafetyStore.getState().load();
    }
  };

  const problems = findings?.filter((f) => f.level === "warn").length ?? 0;

  return (
    <section aria-labelledby="check-heading">
      <div className="flex items-center justify-between gap-3">
        <h2 id="check-heading" className="text-[19px] font-semibold">
          System check
          {findings && (
            <span className={cn("ml-2 text-[13px] font-normal", problems ? "text-moderate" : "text-safe")}>
              {problems ? `${problems} to look at` : "All clear"}
            </span>
          )}
        </h2>
        <Button variant="ghost" size="icon" onClick={() => void run()} aria-label="Run the system check again">
          <RefreshCw />
        </Button>
      </div>
      {!findings ? (
        <div className="mt-3 space-y-2">
          {Array.from({ length: 4 }, (_, i) => (
            <Skeleton key={i} className="h-10 w-full" />
          ))}
        </div>
      ) : (
        <ul className="mt-3 divide-y divide-line border-y border-line">
          <AnimatePresence initial={false}>
            {findings.map((f) => (
              <motion.li
                key={f.id}
                layout
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="flex items-start gap-3 py-2.5"
              >
                <span className="mt-0.5">{icon[f.level]}</span>
                <div className="min-w-0 flex-1">
                  <p>{f.title}</p>
                  <p className="text-[12px] text-muted">{f.detail}</p>
                </div>
                {f.fix && (
                  <Button size="sm" variant="primary" disabled={busy} onClick={() => void fix(f)}>
                    Switch to {f.fix.hz} Hz
                  </Button>
                )}
              </motion.li>
            ))}
          </AnimatePresence>
        </ul>
      )}
    </section>
  );
}
