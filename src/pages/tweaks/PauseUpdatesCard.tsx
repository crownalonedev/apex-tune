import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { api, toAppError, type PauseStatus } from "@/lib/tauri";
import { cn } from "@/lib/utils";

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "long" });

export function PauseUpdatesCard() {
  const [status, setStatus] = useState<PauseStatus | null>(null);
  const [weeks, setWeeks] = useState(1);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api.updatesPauseStatus().then(setStatus).catch(() => {});
  }, []);

  const run = async (action: () => Promise<PauseStatus>, done: string) => {
    setBusy(true);
    try {
      setStatus(await action());
      toast.success(done);
    } catch (e) {
      toast.error(toAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="rounded-lg border border-line bg-panel p-4">
      <h3 className="text-[17px] font-semibold">Pause Windows Update</h3>
      <p className="mt-1 max-w-[72ch] text-[13px] text-muted">
        The same pause as Settings → Windows Update, for 1 to 5 weeks. Updates resume on their own afterwards.
        Apex Tune never turns Windows Update off.
      </p>
      {status?.until ? (
        <div className="mt-3 flex flex-wrap items-center gap-3">
          <span className="text-signal">Paused until {dateFormat.format(new Date(status.until))}.</span>
          {status.ours && (
            <Button size="sm" variant="ghost" disabled={busy} onClick={() => void run(api.updatesResume, "Windows Update resumed")}>
              Resume now
            </Button>
          )}
        </div>
      ) : (
        <div className="mt-3 flex flex-wrap items-center gap-2">
          {[1, 2, 3, 4, 5].map((w) => (
            <button
              key={w}
              type="button"
              aria-pressed={weeks === w}
              onClick={() => setWeeks(w)}
              className={cn(
                "h-8 rounded-md border px-3 text-sm",
                weeks === w ? "border-signal bg-signal-soft text-signal" : "border-line-strong text-muted",
              )}
            >
              {w} {w === 1 ? "week" : "weeks"}
            </button>
          ))}
          <Button size="sm" variant="primary" disabled={busy} onClick={() => void run(() => api.updatesPause(weeks), `Updates paused for ${weeks} week${weeks > 1 ? "s" : ""}`)}>
            Pause
          </Button>
        </div>
      )}
    </section>
  );
}
