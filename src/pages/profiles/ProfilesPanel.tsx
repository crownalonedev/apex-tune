import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { save, open } from "@tauri-apps/plugin-dialog";
import { Download, Trash2, Upload } from "lucide-react";
import { toast } from "sonner";
import { restartAction } from "@/components/badges";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { plural } from "@/lib/format";
import { api, inTauri, toAppError, type ProfileView } from "@/lib/tauri";
import { useAppStore } from "@/stores/app";
import { useSafetyStore } from "@/stores/safety";
import { useTweaksStore } from "@/stores/tweaks";

export function ProfilesPanel() {
  const [profiles, setProfiles] = useState<ProfileView[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [applying, setApplying] = useState<ProfileView | null>(null);
  const [exclusive, setExclusive] = useState(false);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState("");

  const load = () =>
    api
      .profilesList()
      .then(setProfiles)
      .catch((e) => toast.error(toAppError(e).message));

  useEffect(() => {
    void load();
  }, []);

  const guard = async (work: () => Promise<void>) => {
    setBusy(true);
    try {
      await work();
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 12_000 });
    } finally {
      setBusy(false);
      await load();
    }
  };

  const apply = (p: ProfileView) =>
    guard(async () => {
      const report = await api.profileApply(p.id, exclusive);
      useAppStore.getState().needRestart(report.restart);
      const failed = report.results.filter((r) => r.status === "failed");
      if (failed.length) {
        toast.warning(`${p.name}: ${failed.length} couldn't be changed and ${failed.length === 1 ? "was" : "were"} undone`, {
          description: failed.map((f) => `${f.name}: ${f.error}`).join("\n"),
          duration: 15_000,
        });
      } else {
        toast.success(`Applied ${p.name}`, {
          description: report.restart !== "none" ? `${restartAction[report.restart]} to finish.` : undefined,
        });
      }
      void useTweaksStore.getState().load();
      void useSafetyStore.getState().load();
    });

  const exportProfile = (p: ProfileView) =>
    guard(async () => {
      if (!inTauri()) throw { kind: "other", message: "Not available in the browser preview." };
      const path = await save({ defaultPath: `${p.name}.apextune.json`, filters: [{ name: "Apex Tune profile", extensions: ["json"] }] });
      if (!path) return;
      await api.profileExport(p.id, path);
      toast.success(`Exported ${p.name}`);
    });

  const importProfile = () =>
    guard(async () => {
      if (!inTauri()) throw { kind: "other", message: "Not available in the browser preview." };
      const path = await open({ multiple: false, filters: [{ name: "Apex Tune profile", extensions: ["json"] }] });
      if (!path || Array.isArray(path)) return;
      const { profile, dropped } = await api.profileImport(path);
      toast.success(`Imported ${profile.name}`, {
        description: dropped.length ? `${plural(dropped.length, "tweak")} from a different version left out.` : undefined,
      });
    });

  const saveCurrent = () =>
    guard(async () => {
      const p = await api.profileSave(name);
      setNaming(false);
      setName("");
      toast.success(`Saved ${p.name}`);
    });

  return (
    <div className="mt-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="max-w-[60ch] text-muted">A profile is a set of tweaks. Applying one turns its tweaks on; everything stays revertible.</p>
        <span className="flex gap-2">
          <Button onClick={() => void importProfile()} disabled={busy}>
            <Upload /> Import
          </Button>
          <Button variant="primary" onClick={() => setNaming(true)} disabled={busy}>
            Save current setup
          </Button>
        </span>
      </div>

      {!profiles ? (
        <Skeleton className="mt-4 h-40 w-full" />
      ) : (
        <ul className="mt-4 grid gap-3 md:grid-cols-2">
          {profiles.map((p, i) => {
            const pct = p.tweaks.length ? p.applied / p.tweaks.length : 0;
            return (
              <motion.li
                key={p.id}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: i * 0.04 }}
                className="flex flex-col rounded-lg border border-line bg-panel p-4"
              >
                <div className="flex items-start justify-between gap-2">
                  <h3 className="text-[17px] font-semibold">{p.name}</h3>
                  {p.builtIn && <Badge>Built in</Badge>}
                </div>
                <p className="mt-1 flex-1 text-[13px] text-muted">{p.description}</p>
                <div className="mt-3" aria-label={`${p.applied} of ${p.tweaks.length} tweaks on`}>
                  <div className="h-1.5 overflow-hidden rounded-full bg-panel-raised">
                    <motion.div className="h-full bg-signal" initial={{ width: 0 }} animate={{ width: `${pct * 100}%` }} />
                  </div>
                  <p className="mt-1 text-[12px] text-faint">
                    {p.applied} of {plural(p.tweaks.length, "tweak")} on
                  </p>
                </div>
                <div className="mt-3 flex flex-wrap gap-2">
                  <Button size="sm" variant="primary" disabled={busy || p.applied === p.tweaks.length} onClick={() => setApplying(p)}>
                    {p.applied === p.tweaks.length ? "All on" : "Apply"}
                  </Button>
                  <Button size="sm" variant="ghost" disabled={busy} onClick={() => void exportProfile(p)}>
                    <Download /> Export
                  </Button>
                  {!p.builtIn && (
                    <Button size="sm" variant="ghost" disabled={busy} onClick={() => void guard(() => api.profileDelete(p.id))} aria-label={`Delete ${p.name}`}>
                      <Trash2 />
                    </Button>
                  )}
                </div>
              </motion.li>
            );
          })}
        </ul>
      )}

      <AlertDialog open={applying !== null} onOpenChange={(o) => !o && setApplying(null)}>
        <AlertDialogContent>
          {applying && (
            <>
              <AlertDialogTitle>Apply {applying.name}?</AlertDialogTitle>
              <AlertDialogDescription>
                Turns on {plural(applying.tweaks.length - applying.applied, "tweak")}. A restore point is made first if this session doesn't have one.
              </AlertDialogDescription>
              <label className="mt-3 flex items-start gap-2 text-[13px]">
                <input type="checkbox" checked={exclusive} onChange={(e) => setExclusive(e.target.checked)} className="mt-0.5 accent-[var(--signal)]" />
                Also turn off tweaks Apex Tune applied that aren't in this profile
              </label>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction variant="primary" onClick={() => void apply(applying)}>
                  Apply
                </AlertDialogAction>
              </AlertDialogFooter>
            </>
          )}
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog open={naming} onOpenChange={setNaming}>
        <AlertDialogContent>
          <AlertDialogTitle>Save the current setup</AlertDialogTitle>
          <AlertDialogDescription>Every tweak that's on right now goes into the profile.</AlertDialogDescription>
          <label className="mt-3 block text-[13px]" htmlFor="profile-name">
            Name
          </label>
          <input
            id="profile-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={60}
            className="mt-1 h-9 w-full rounded-md border border-line-strong bg-bg px-3"
            autoFocus
          />
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction variant="primary" disabled={!name.trim()} onClick={() => void saveCurrent()}>
              Save
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
