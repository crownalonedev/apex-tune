import { useEffect } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { formatWmiDate, plural } from "@/lib/format";
import type { SafetyStatus } from "@/lib/tauri";
import { useSafetyStore } from "@/stores/safety";

/** Restore point state and Revert Everything. */
export function SafetyPanel() {
  const { status, error, busy, load } = useSafetyStore();

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <section aria-labelledby="safety-heading">
      <h2 id="safety-heading" className="text-[19px] font-semibold">
        Safety net
      </h2>
      <p className="mt-1 max-w-[68ch] text-muted">
        Every change is backed up before it's made and can be undone, one at a time or all at once.
      </p>

      <dl className="mt-3 divide-y divide-line border-y border-line">
        <div className="flex flex-wrap items-center gap-x-6 gap-y-2 py-3">
          <dt className="w-40 shrink-0 text-muted">Restore point</dt>
          <dd className="min-w-0 flex-1">{status ? <RestorePointState status={status} /> : <Skeleton className="h-4 w-64" />}</dd>
        </div>
        <div className="flex flex-wrap items-center gap-x-6 gap-y-2 py-3">
          <dt className="w-40 shrink-0 text-muted">Changes</dt>
          <dd className="flex min-w-0 flex-1 flex-wrap items-center justify-between gap-3">
            {status ? (
              <>
                <span>
                  {status.activeChanges === 0
                    ? "Nothing changed yet."
                    : `${plural(status.activeChanges, "change")} can be reverted.`}
                </span>
                <RevertEverything count={status.activeChanges} disabled={busy !== null} />
              </>
            ) : (
              <Skeleton className="h-4 w-40" />
            )}
          </dd>
        </div>
      </dl>

      {status && status.unreadableBackups.length > 0 && (
        <p className="mt-3 text-[13px] text-moderate">
          {plural(status.unreadableBackups.length, "backup file")} couldn't be read and {status.unreadableBackups.length === 1 ? "was" : "were"} left in place:{" "}
          <span className="selectable break-all">{status.unreadableBackups.join(", ")}</span>
        </p>
      )}
      {error && (
        <p role="alert" className="mt-3 text-[13px] text-danger">
          {error.message}
        </p>
      )}
    </section>
  );
}

function RestorePointState({ status }: { status: SafetyStatus }) {
  const { busy, createRestorePoint, enableProtection, skipRestorePoint } = useSafetyStore();

  if (status.restorePoint) {
    return (
      <span>
        Created {formatWmiDate(status.restorePoint.created)} for this session.
      </span>
    );
  }
  if (status.restorePointSkipped) {
    return <span className="text-moderate">Skipped for this session. Changes are still backed up individually.</span>;
  }
  if (status.protectionEnabled === false) {
    return (
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="text-moderate">System Protection is off, so Windows can't make restore points.</span>
        <span className="flex gap-2">
          <Button size="sm" variant="ghost" onClick={() => void skipRestorePoint()} disabled={busy !== null}>
            Continue without
          </Button>
          <Button size="sm" variant="primary" onClick={() => void enableProtection()} disabled={busy !== null}>
            {busy === "protection" || busy === "restorePoint" ? "Turning on…" : "Turn on System Protection"}
          </Button>
        </span>
      </div>
    );
  }
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <span>None yet. One is made automatically before your first change.</span>
      <Button size="sm" onClick={() => void createRestorePoint()} disabled={busy !== null}>
        {busy === "restorePoint" ? "Creating…" : "Create now"}
      </Button>
    </div>
  );
}

function RevertEverything({ count, disabled }: { count: number; disabled: boolean }) {
  const revertEverything = useSafetyStore((s) => s.revertEverything);
  const busy = useSafetyStore((s) => s.busy === "revertAll");

  const onConfirm = async () => {
    try {
      const report = await revertEverything();
      if (report.failed.length === 0) {
        toast.success(`Reverted ${plural(report.reverted.length, "change")}`);
      } else {
        toast.warning(`Reverted ${report.reverted.length}, ${report.failed.length} couldn't be reverted`, {
          description: report.failed.map((f) => `${f.tweak}: ${f.message}`).join("\n"),
          duration: 15_000,
        });
      }
    } catch {
      // The store shows the error inline.
    }
  };

  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button size="sm" variant="danger" disabled={disabled || count === 0}>
          {busy ? "Reverting…" : "Revert everything"}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogTitle>Revert {plural(count, "change")}?</AlertDialogTitle>
        <AlertDialogDescription>
          Apex Tune puts back the original value of every setting it changed, newest first. Some settings
          only return fully after a restart.
        </AlertDialogDescription>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction variant="danger" onClick={() => void onConfirm()}>
            Revert everything
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
