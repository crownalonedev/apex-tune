import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
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
import { plural } from "@/lib/format";
import { reportResults } from "@/lib/results";
import { api, toAppError, type AppRow } from "@/lib/tauri";
import { toast } from "sonner";
import { useSafetyStore } from "@/stores/safety";

export function AppsPanel() {
  const [rows, setRows] = useState<AppRow[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  const load = () =>
    api
      .debloatApps()
      .then(setRows)
      .catch((e) => toast.error(toAppError(e).message));

  useEffect(() => {
    void load();
  }, []);

  const groups = useMemo(() => {
    const r = rows ?? [];
    return {
      removable: r.filter((a) => a.installed && a.group === "removable"),
      optional: r.filter((a) => a.installed && a.group === "optional"),
      removed: r.filter((a) => !a.installed && a.restorable),
      keep: r.filter((a) => a.installed && a.group === "keep"),
      other: r.filter((a) => a.installed && a.group === "other"),
    };
  }, [rows]);

  const toggle = (name: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });

  const remove = async () => {
    setBusy(true);
    try {
      reportResults(await api.debloatRemoveApps([...selected]), "Removed", "app");
      setSelected(new Set());
    } catch (e) {
      toast.error(toAppError(e).message);
    } finally {
      setBusy(false);
      await load();
      void useSafetyStore.getState().load();
    }
  };

  const restore = async (row: AppRow) => {
    setBusy(true);
    try {
      await api.debloatRestoreApp(row.name);
      toast.success(`${row.displayName} is back`);
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 15_000 });
    } finally {
      setBusy(false);
      await load();
    }
  };

  if (!rows) {
    return (
      <div className="mt-4 space-y-2" aria-busy="true">
        {Array.from({ length: 6 }, (_, i) => (
          <Skeleton key={i} className="h-10 w-full" />
        ))}
      </div>
    );
  }

  return (
    <div className="mt-5 space-y-8 pb-20">
      <p className="max-w-[72ch] text-muted">
        Apps are removed for your account only. Windows keeps its copy, so anything removed here can be brought
        back from the "Removed by Apex Tune" list, offline. The Store, Xbox sign-in, Game Bar and Gaming Services
        are never offered: games need them.
      </p>

      <Group title="Safe to remove" rows={groups.removable} selected={selected} onToggle={toggle} selectAll={() => setSelected(new Set([...selected, ...groups.removable.map((r) => r.name)]))} />
      <Group title="Optional" rows={groups.optional} selected={selected} onToggle={toggle} />

      {groups.removed.length > 0 && (
        <section>
          <h3 className="text-[17px] font-semibold">Removed by Apex Tune</h3>
          <ul className="mt-2 divide-y divide-line border-y border-line">
            {groups.removed.map((r) => (
              <li key={r.name} className="flex items-center justify-between gap-3 py-2.5">
                <span>{r.displayName}</span>
                <Button size="sm" variant="ghost" disabled={busy} onClick={() => void restore(r)}>
                  Bring back
                </Button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <details className="group">
        <summary className="cursor-pointer text-[17px] font-semibold">
          Kept on purpose <span className="text-[13px] font-normal text-faint">{groups.keep.length}</span>
        </summary>
        <ReadOnlyList rows={groups.keep} />
      </details>
      <details>
        <summary className="cursor-pointer text-[17px] font-semibold">
          Other installed apps <span className="text-[13px] font-normal text-faint">{groups.other.length}</span>
        </summary>
        <p className="mt-2 text-[13px] text-muted">
          Not reviewed by Apex Tune. Uninstall these from Windows Settings if you want them gone.
        </p>
        <ReadOnlyList rows={groups.other} />
      </details>

      <AnimatePresence>
        {selected.size > 0 && (
          <motion.div
            initial={{ y: 24, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 24, opacity: 0 }}
            className="sticky bottom-4 flex items-center justify-between gap-3 rounded-lg border border-line-strong bg-panel-raised px-4 py-3 shadow-xl shadow-black/30"
          >
            <span>{plural(selected.size, "app")} selected</span>
            <span className="flex gap-2">
              <Button variant="ghost" onClick={() => setSelected(new Set())}>
                Clear
              </Button>
              <AlertDialog>
                <AlertDialogTrigger asChild>
                  <Button variant="primary" disabled={busy}>
                    {busy ? "Removing…" : `Remove ${plural(selected.size, "app")}`}
                  </Button>
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogTitle>Remove {plural(selected.size, "app")}?</AlertDialogTitle>
                  <AlertDialogDescription>
                    They're removed for your account. You can bring each one back from this page. An app you
                    installed yourself (not one that came with Windows) may need reinstalling from the Store instead.
                  </AlertDialogDescription>
                  <ul className="mt-3 max-h-40 overflow-y-auto text-[13px] text-muted">
                    {rows.filter((r) => selected.has(r.name)).map((r) => (
                      <li key={r.name}>{r.displayName}</li>
                    ))}
                  </ul>
                  <AlertDialogFooter>
                    <AlertDialogCancel>Cancel</AlertDialogCancel>
                    <AlertDialogAction variant="primary" onClick={() => void remove()}>
                      Remove
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </span>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function Group({
  title,
  rows,
  selected,
  onToggle,
  selectAll,
}: {
  title: string;
  rows: AppRow[];
  selected: Set<string>;
  onToggle: (name: string) => void;
  selectAll?: () => void;
}) {
  if (rows.length === 0) return null;
  return (
    <section>
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-[17px] font-semibold">
          {title} <span className="text-[13px] font-normal text-faint">{rows.length}</span>
        </h3>
        {selectAll && (
          <Button size="sm" variant="ghost" onClick={selectAll}>
            Select all
          </Button>
        )}
      </div>
      <ul className="mt-2 grid gap-px overflow-hidden rounded-lg border border-line bg-line sm:grid-cols-2">
        {rows.map((r) => {
          const id = `app-${r.name}`;
          return (
            <li key={r.name} className="flex gap-3 bg-panel px-3 py-2.5">
              <input
                id={id}
                type="checkbox"
                checked={selected.has(r.name)}
                onChange={() => onToggle(r.name)}
                className="mt-1 size-4 shrink-0 accent-[var(--signal)]"
              />
              <label htmlFor={id} className="min-w-0 flex-1 cursor-pointer">
                <span className="block">{r.displayName}</span>
                <span className="block text-[12px] text-muted">{r.note}</span>
              </label>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function ReadOnlyList({ rows }: { rows: AppRow[] }) {
  return (
    <ul className="mt-2 divide-y divide-line border-y border-line text-[13px]">
      {rows.map((r) => (
        <li key={r.name} className="flex justify-between gap-4 py-2">
          <span>{r.displayName}</span>
          <span className="text-right text-muted">{r.note}</span>
        </li>
      ))}
    </ul>
  );
}
