import { useEffect, useState } from "react";
import { toast } from "sonner";
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
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { formatBytes } from "@/lib/format";
import { api, toAppError, type CleanTargetInfo } from "@/lib/tauri";

export function StoragePanel() {
  const [targets, setTargets] = useState<CleanTargetInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [freed, setFreed] = useState(0);

  const scan = () =>
    api
      .storageScan()
      .then(setTargets)
      .catch((e) => toast.error(toAppError(e).message));

  useEffect(() => {
    void scan();
  }, []);

  const clean = async (t: CleanTargetInfo) => {
    setBusy(t.target);
    try {
      const r = await api.storageClean(t.target);
      setFreed((f) => f + r.freedBytes);
      toast.success(`Freed ${formatBytes(r.freedBytes)}`, {
        description: r.skippedFiles ? `${r.skippedFiles} files were in use and left alone.` : undefined,
      });
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 12_000 });
    } finally {
      setBusy(null);
      await scan();
    }
  };

  const total = targets?.reduce((sum, t) => sum + t.bytes, 0) ?? 0;

  return (
    <div className="mt-5">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <p className="max-w-[72ch] text-muted">
          Deleting cached files can't be undone, so each cleanup shows its size first and asks before it runs.
          Files in use are skipped.
        </p>
        <p className="text-right">
          <span className="block text-[13px] text-muted">Can be freed</span>
          <span className="text-[24px] font-semibold tabular-nums [font-stretch:80%]">{formatBytes(total)}</span>
          {freed > 0 && <span className="block text-[12px] text-signal">{formatBytes(freed)} freed this session</span>}
        </p>
      </div>
      {!targets ? (
        <Skeleton className="mt-4 h-48 w-full" />
      ) : (
        <ul className="mt-4 divide-y divide-line border-y border-line">
          {targets.map((t) => (
            <li key={t.target} className="flex flex-wrap items-center gap-4 py-3">
              <div className="min-w-0 flex-1 basis-72">
                <span className="block">{t.name}</span>
                <span className="block text-[12px] text-muted">{t.description}</span>
                {t.warning && <span className="block text-[12px] text-moderate">{t.warning}</span>}
              </div>
              <span className="w-24 text-right tabular-nums">{formatBytes(t.bytes)}</span>
              <AlertDialog>
                <AlertDialogTrigger asChild>
                  <Button size="sm" disabled={busy !== null || t.bytes === 0}>
                    {busy === t.target ? "Cleaning…" : "Clean"}
                  </Button>
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogTitle>Delete {formatBytes(t.bytes)} of {t.name.toLowerCase()}?</AlertDialogTitle>
                  <AlertDialogDescription>
                    {t.files.toLocaleString()} files. This can't be undone. {t.warning ?? ""}
                  </AlertDialogDescription>
                  <AlertDialogFooter>
                    <AlertDialogCancel>Cancel</AlertDialogCancel>
                    <AlertDialogAction variant="danger" onClick={() => void clean(t)}>
                      Delete
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
