import { useEffect, useState } from "react";
import { toast } from "sonner";
import { RiskBadge } from "@/components/badges";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { plural } from "@/lib/format";
import { reportResults } from "@/lib/results";
import { api, toAppError, type Background, type StartType } from "@/lib/tauri";

const startLabel: Record<StartType, string> = {
  boot: "Boot",
  system: "System",
  automatic: "Automatic",
  automaticDelayed: "Automatic (delayed)",
  manual: "Manual",
  disabled: "Disabled",
};

export function BackgroundPanel() {
  const [data, setData] = useState<Background | null>(null);
  const [tasks, setTasks] = useState<Set<string>>(new Set());
  const [services, setServices] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  const load = () =>
    api
      .debloatBackground()
      .then((d) => {
        setData(d);
        // Pre-select what still needs doing, except Moderate services.
        setTasks(new Set(d.tasks.filter((t) => t.enabled).map((t) => t.path)));
        setServices(
          new Set(d.services.filter((s) => s.current && s.current !== s.recommended && s.risk === "safe").map((s) => s.name)),
        );
      })
      .catch((e) => toast.error(toAppError(e).message));

  useEffect(() => {
    void load();
  }, []);

  const flip = (set: Set<string>, key: string) => {
    const next = new Set(set);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    return next;
  };

  const apply = async () => {
    setBusy(true);
    try {
      reportResults(await api.debloatApplyBackground([...tasks], [...services]), "Changed", "item");
    } catch (e) {
      toast.error(toAppError(e).message);
    } finally {
      setBusy(false);
      await load();
    }
  };

  const restore = async (key: string) => {
    setBusy(true);
    try {
      await api.debloatRestoreBackground(key);
      toast.success("Restored");
    } catch (e) {
      toast.error(toAppError(e).message);
    } finally {
      setBusy(false);
      await load();
    }
  };

  if (!data) return <Skeleton className="mt-5 h-48 w-full" />;
  const changed = new Set(data.changed);
  const count = tasks.size + services.size;

  return (
    <div className="mt-5 space-y-8">
      <section>
        <h3 className="text-[17px] font-semibold">Telemetry and consumer tasks</h3>
        <p className="mt-1 max-w-[72ch] text-[13px] text-muted">
          Disabled, not deleted. A few are owned by Windows itself and refuse to change; those are reported and
          left alone.
        </p>
        <ul className="mt-3 divide-y divide-line border-y border-line">
          {data.tasks.map((t) => {
            const id = `task-${t.path}`;
            return (
              <li key={t.path} className="flex items-start gap-3 py-2.5">
                <input
                  id={id}
                  type="checkbox"
                  checked={tasks.has(t.path)}
                  disabled={t.enabled !== true}
                  onChange={() => setTasks(flip(tasks, t.path))}
                  className="mt-1 size-4 shrink-0 accent-[var(--signal)]"
                />
                <label htmlFor={id} className="min-w-0 flex-1">
                  <span className="block">{t.name}</span>
                  <span className="block text-[12px] text-muted">{t.description}</span>
                </label>
                <span className="shrink-0 text-[12px] text-faint">
                  {t.enabled === null ? "Not on this PC" : t.enabled ? "Enabled" : "Disabled"}
                </span>
                {changed.has(t.path) && (
                  <Button size="sm" variant="ghost" disabled={busy} onClick={() => void restore(t.path)}>
                    Restore
                  </Button>
                )}
              </li>
            );
          })}
        </ul>
      </section>

      <section>
        <h3 className="text-[17px] font-semibold">Services</h3>
        <p className="mt-1 max-w-[72ch] text-[13px] text-muted">
          A deliberately short list. Services games, updates or security depend on are never offered.
        </p>
        <ul className="mt-3 divide-y divide-line border-y border-line">
          {data.services.map((s) => {
            const id = `svc-${s.name}`;
            const done = s.current === s.recommended;
            return (
              <li key={s.name} className="flex items-start gap-3 py-2.5">
                <input
                  id={id}
                  type="checkbox"
                  checked={services.has(s.name)}
                  disabled={!s.current || done}
                  onChange={() => setServices(flip(services, s.name))}
                  className="mt-1 size-4 shrink-0 accent-[var(--signal)]"
                />
                <label htmlFor={id} className="min-w-0 flex-1">
                  <span className="flex flex-wrap items-center gap-2">
                    {s.display}
                    <RiskBadge risk={s.risk} />
                  </span>
                  <span className="block text-[12px] text-muted">{s.description}</span>
                </label>
                <span className="shrink-0 text-right text-[12px] text-faint">
                  {s.current ? (
                    <>
                      {startLabel[s.current]}
                      {!done && <> → {startLabel[s.recommended]}</>}
                    </>
                  ) : (
                    "Not on this PC"
                  )}
                </span>
                {changed.has(s.name) && (
                  <Button size="sm" variant="ghost" disabled={busy} onClick={() => void restore(s.name)}>
                    Restore
                  </Button>
                )}
              </li>
            );
          })}
        </ul>
      </section>

      <div className="flex justify-end">
        <Button variant="primary" disabled={busy || count === 0} onClick={() => void apply()}>
          {busy ? "Applying…" : `Apply ${plural(count, "change")}`}
        </Button>
      </div>
    </div>
  );
}
