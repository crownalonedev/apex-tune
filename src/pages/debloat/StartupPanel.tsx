import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { api, toAppError, type StartupItem, type StartupSource } from "@/lib/tauri";
import { cn } from "@/lib/utils";

const sourceLabel: Record<StartupSource, string> = {
  userRun: "Your account",
  machineRun: "All users",
  machineRun32: "All users (32-bit)",
  userFolder: "Startup folder",
  commonFolder: "Startup folder (all users)",
  task: "Scheduled task",
};

export function StartupPanel() {
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = () =>
    api
      .debloatStartup()
      .then(setItems)
      .catch((e) => toast.error(toAppError(e).message));

  useEffect(() => {
    void load();
  }, []);

  const set = async (item: StartupItem, enabled: boolean) => {
    setBusy(item.id);
    try {
      await api.debloatSetStartup(item.id, enabled);
      toast.success(`${item.name} ${enabled ? "starts with Windows again" : "no longer starts with Windows"}`);
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 12_000 });
    } finally {
      setBusy(null);
      await load();
    }
  };

  if (!items) return <Skeleton className="mt-5 h-48 w-full" />;
  const on = items.filter((i) => i.enabled).length;

  return (
    <div className="mt-5">
      <p className="max-w-[72ch] text-muted">
        {on} of {items.length} start when you sign in. Switching one off works like Task Manager: the entry stays,
        Windows just skips it, and you can switch it back on any time.
      </p>
      <ul className="mt-4 divide-y divide-line border-y border-line">
        {items.map((item) => (
          <li key={item.id} className="flex items-center gap-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className={cn(!item.enabled && "text-muted")}>{item.name}</span>
                <Badge>{sourceLabel[item.source]}</Badge>
              </div>
              <p className="selectable truncate text-[12px] text-faint" title={item.command}>
                {item.command}
              </p>
              {item.protected && <p className="text-[12px] text-moderate">{item.protected}</p>}
            </div>
            <Switch
              checked={item.enabled}
              disabled={!!item.protected || busy !== null}
              onCheckedChange={(v) => void set(item, v)}
              aria-label={`${item.name} starts with Windows`}
            />
          </li>
        ))}
      </ul>
    </div>
  );
}
