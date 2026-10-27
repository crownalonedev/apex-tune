import { useEffect, useState } from "react";
import { toast } from "sonner";
import { TweakCard } from "@/components/TweakCard";
import { PageHeader } from "@/components/layout/PageHeader";
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
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { routeById } from "@/lib/routes";
import { api, toAppError, type NvidiaStatus } from "@/lib/tauri";
import { useTweaksStore } from "@/stores/tweaks";

export function NvidiaPage() {
  const route = routeById("nvidia");
  const [status, setStatus] = useState<NvidiaStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const loadTweaks = useTweaksStore((s) => s.load);
  const global = useTweaksStore((s) => s.overview?.tweaks.find((t) => t.id === "nvidia_global"));

  useEffect(() => {
    void loadTweaks();
    api
      .nvidiaStatus()
      .then(setStatus)
      .catch((e) => toast.error(toAppError(e).message));
  }, [loadTweaks]);

  const run = async (action: () => Promise<NvidiaStatus>, done: string) => {
    setBusy(true);
    try {
      setStatus(await action());
      toast.success(done);
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 12_000 });
    } finally {
      setBusy(false);
    }
  };

  const restore = async () => {
    setBusy(true);
    try {
      await api.nvidiaRestoreBackup();
      toast.success("All NVIDIA settings restored from the backup");
    } catch (e) {
      toast.error(toAppError(e).message);
    } finally {
      setBusy(false);
      void loadTweaks();
    }
  };

  return (
    <>
      <PageHeader
        title={route.label}
        summary={route.summary}
        actions={status?.info && <span className="text-[13px] text-muted">{status.info.gpu}, driver {status.info.driver}</span>}
      />

      {!status ? (
        <Skeleton className="mt-8 h-40 w-full" />
      ) : !status.info ? (
        <section className="mt-10 max-w-[60ch]">
          <h2 className="text-[19px] font-semibold">No NVIDIA GPU found</h2>
          <p className="mt-2 text-muted">
            This page manages NVIDIA driver profiles through NVIDIA's own API, which needs an NVIDIA graphics card and
            driver. Everything else in Apex Tune works without one.
          </p>
        </section>
      ) : (
        <div className="mt-8 space-y-10">
          {global && (
            <section>
              <h2 className="mb-3 text-[19px] font-semibold">Global profile</h2>
              <TweakCard tweak={global} />
            </section>
          )}

          <section>
            <h2 className="text-[19px] font-semibold">Per-game profiles</h2>
            <p className="mt-1 max-w-[72ch] text-muted">
              Each game gets maximum-performance power and fast texture filtering. Low Latency Mode is added only for
              games without NVIDIA Reflex; for Reflex games, turn on Reflex + Boost in the game's own settings. A
              frame cap a few FPS below your refresh rate keeps G-SYNC active and latency low.
            </p>
            <ul className="mt-4 divide-y divide-line border-y border-line">
              {status.games.map((g) => (
                <li key={g.exe} className="flex flex-wrap items-center gap-x-6 gap-y-3 py-3">
                  <div className="min-w-0 flex-1 basis-60">
                    <span className="flex flex-wrap items-center gap-2">
                      {g.name}
                      {g.reflex && <Badge tone="signal">Reflex: use the in-game setting</Badge>}
                    </span>
                    <span className="block text-[12px] text-faint">{g.exe}</span>
                  </div>
                  <label className="flex items-center gap-2 text-[13px]">
                    <Switch
                      checked={g.applied}
                      disabled={busy}
                      onCheckedChange={(v) =>
                        void run(() => api.nvidiaSetGame(g.exe, v), `${g.name}: profile ${v ? "applied" : "removed"}`)
                      }
                      aria-label={`Low-latency profile for ${g.name}`}
                    />
                    Low-latency profile
                  </label>
                  <FpsCap
                    current={g.fpsCap}
                    disabled={busy}
                    onSet={(fps) => void run(() => api.nvidiaSetFps(g.exe, fps), fps ? `${g.name} capped at ${fps} FPS` : `${g.name}: frame cap removed`)}
                    label={g.name}
                  />
                </li>
              ))}
            </ul>
          </section>

          <section className="max-w-[72ch]">
            <h2 className="text-[19px] font-semibold">Full backup</h2>
            <p className="mt-1 text-muted">
              Before its first NVIDIA change in a session, Apex Tune saves the entire driver settings database. Each
              change can also be reverted on its own from Profiles & backups.
            </p>
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button className="mt-3" variant="danger" disabled={busy || !status.hasBackup}>
                  Restore all NVIDIA settings from backup
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogTitle>Restore every NVIDIA setting?</AlertDialogTitle>
                <AlertDialogDescription>
                  The driver's settings go back to exactly how they were in the newest backup, including changes made
                  in NVIDIA Control Panel since then.
                </AlertDialogDescription>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction variant="danger" onClick={() => void restore()}>
                    Restore
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
            {!status.hasBackup && <p className="mt-2 text-[12px] text-faint">No backup yet; one is made before your first NVIDIA change.</p>}
          </section>
        </div>
      )}
    </>
  );
}

function FpsCap({ current, disabled, onSet, label }: { current: number | null; disabled: boolean; onSet: (fps: number) => void; label: string }) {
  const [value, setValue] = useState(current?.toString() ?? "");
  useEffect(() => setValue(current?.toString() ?? ""), [current]);
  const n = Number(value);
  const valid = value !== "" && Number.isInteger(n) && n >= 20 && n <= 1000;

  return (
    <form
      className="flex items-center gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        if (valid) onSet(n);
      }}
    >
      <label className="text-[13px]" htmlFor={`fps-${label}`}>
        Frame cap
      </label>
      <input
        id={`fps-${label}`}
        inputMode="numeric"
        placeholder="Off"
        value={value}
        onChange={(e) => setValue(e.target.value.replace(/\D/g, ""))}
        className="h-8 w-16 rounded-md border border-line-strong bg-bg px-2 text-sm tabular-nums"
      />
      <Button size="sm" type="submit" disabled={disabled || !valid || n === current}>
        Set
      </Button>
      {current !== null && (
        <Button size="sm" variant="ghost" type="button" disabled={disabled} onClick={() => onSet(0)}>
          Remove
        </Button>
      )}
    </form>
  );
}
