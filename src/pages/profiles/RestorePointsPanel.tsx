import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { formatWmiDate } from "@/lib/format";
import { api, toAppError, type RestorePoint } from "@/lib/tauri";

export function RestorePointsPanel() {
  const [points, setPoints] = useState<RestorePoint[] | null>(null);

  useEffect(() => {
    api
      .listRestorePoints()
      .then((p) => setPoints([...p].reverse()))
      .catch((e) => {
        setPoints([]);
        toast.error(toAppError(e).message);
      });
  }, []);

  const openWizard = () => api.openSystemRestore().catch((e) => toast.error(toAppError(e).message));

  return (
    <div className="mt-5 max-w-3xl">
      <p className="text-muted">
        A restore point rolls the whole system back, including drivers and changes Apex Tune didn't make. Use it if
        Windows won't behave after a change; for single settings, reverting from "What changed" is quicker. Windows
        restarts to complete a restore, so Apex Tune hands you over to its own System Restore wizard.
      </p>
      <Button className="mt-3" onClick={() => void openWizard()}>
        Open System Restore
      </Button>
      {!points ? (
        <Skeleton className="mt-4 h-24 w-full" />
      ) : points.length === 0 ? (
        <p className="mt-4 text-muted">No restore points on this PC.</p>
      ) : (
        <ul className="mt-4 divide-y divide-line border-y border-line">
          {points.map((p) => (
            <li key={p.sequence} className="flex justify-between gap-4 py-2.5">
              <span>{p.description}</span>
              <span className="text-[13px] text-muted">{formatWmiDate(p.created)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
