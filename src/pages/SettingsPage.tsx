import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Check } from "lucide-react";
import { toast } from "sonner";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { routeById } from "@/lib/routes";
import { api, toAppError, type MotionPref, type ThemeName } from "@/lib/tauri";
import { checkForUpdate, type UpdateResult } from "@/lib/updates";
import { cn } from "@/lib/utils";
import { useActivityStore } from "@/stores/activity";
import { useSettingsStore } from "@/stores/settings";

/** Swatches mirror each theme's tokens so the picker previews the real thing. */
const THEMES: { id: ThemeName; name: string; note: string; bg: string; panel: string; line: string; signal: string; text: string }[] = [
  { id: "graphite", name: "Graphite", note: "Cool slate, ice-blue signal", bg: "#121820", panel: "#18202b", line: "#2a3646", signal: "#7cc4fa", text: "#e8edf3" },
  { id: "ember", name: "Ember", note: "Warm dark, amber signal", bg: "#171311", panel: "#1f1a17", line: "#3a302a", signal: "#f2a65a", text: "#f1e9e2" },
  { id: "moss", name: "Moss", note: "Deep green, mint signal", bg: "#111815", panel: "#161f1b", line: "#283830", signal: "#6fd3a8", text: "#e5efe9" },
  { id: "violet", name: "Violet", note: "Ink purple, lilac signal", bg: "#15131d", panel: "#1c1a26", line: "#332f45", signal: "#b59cff", text: "#ece9f5" },
  { id: "oled", name: "OLED black", note: "True black for OLED screens", bg: "#000000", panel: "#0a0a0a", line: "#222222", signal: "#7cc4fa", text: "#ededed" },
  { id: "daylight", name: "Daylight", note: "Light, for bright rooms", bg: "#eef1f5", panel: "#ffffff", line: "#d6dde6", signal: "#1c7fc4", text: "#16202c" },
  { id: "system", name: "Match Windows", note: "Graphite or Daylight with Windows' mode", bg: "linear-gradient(135deg,#121820 50%,#eef1f5 50%)", panel: "#18202b", line: "#2a3646", signal: "#7cc4fa", text: "#e8edf3" },
];

const MOTION: { id: MotionPref; label: string }[] = [
  { id: "system", label: "Match Windows" },
  { id: "full", label: "Full" },
  { id: "reduced", label: "Reduced" },
];

export function SettingsPage() {
  const route = routeById("settings");
  const { view, load, update, setAutostart } = useSettingsStore();
  const openLog = useActivityStore((s) => s.setOpen);
  const [update_, setUpdate] = useState<UpdateResult | "checking" | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  const check = async () => {
    if (!view) return;
    setUpdate("checking");
    setUpdate(await checkForUpdate(view.version));
  };

  if (!view) {
    return (
      <>
        <PageHeader title={route.label} summary={route.summary} />
        <Skeleton className="mt-8 h-64 w-full" />
      </>
    );
  }

  return (
    <>
      <PageHeader title={route.label} summary={route.summary} />

      <section className="mt-8" aria-labelledby="appearance">
        <h2 id="appearance" className="text-[19px] font-semibold">
          Theme
        </h2>
        <div role="radiogroup" aria-labelledby="appearance" className="mt-3 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {THEMES.map((t) => {
            const active = view.theme === t.id;
            return (
              <button
                key={t.id}
                type="button"
                role="radio"
                aria-checked={active}
                onClick={() => void update({ theme: t.id })}
                className={cn(
                  "group relative overflow-hidden rounded-lg border text-left transition-colors",
                  active ? "border-signal" : "border-line hover:border-line-strong",
                )}
              >
                <ThemePreview t={t} />
                <span className="block px-3 pt-2 pb-2.5">
                  <span className="flex items-center gap-1.5 font-medium">
                    {t.name}
                    {active && (
                      <motion.span initial={{ scale: 0 }} animate={{ scale: 1 }} className="text-signal">
                        <Check className="size-4" aria-hidden="true" />
                      </motion.span>
                    )}
                  </span>
                  <span className="block text-[12px] text-muted">{t.note}</span>
                </span>
              </button>
            );
          })}
        </div>

        <h3 className="mt-6 text-[15px] font-semibold">Motion</h3>
        <div role="radiogroup" aria-label="Motion" className="mt-2 inline-flex rounded-md border border-line-strong p-0.5">
          {MOTION.map((m) => (
            <button
              key={m.id}
              type="button"
              role="radio"
              aria-checked={view.motion === m.id}
              onClick={() => void update({ motion: m.id })}
              className={cn(
                "relative h-8 rounded px-3 text-sm transition-colors",
                view.motion === m.id ? "text-signal-ink" : "text-muted hover:text-text",
              )}
            >
              {view.motion === m.id && (
                <motion.span layoutId="motion-pill" className="absolute inset-0 rounded bg-signal" transition={{ type: "spring", stiffness: 500, damping: 38 }} />
              )}
              <span className="relative">{m.label}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="mt-10 max-w-2xl" aria-labelledby="behaviour">
        <h2 id="behaviour" className="text-[19px] font-semibold">
          Startup and tray
        </h2>
        <ul className="mt-3 divide-y divide-line border-y border-line">
          <Row
            label="Start Apex Tune when I sign in"
            detail="Opens quietly in the tray. Uses a scheduled task, because Windows won't start administrator apps from the usual startup list."
            checked={view.startWithWindows}
            onChange={(v) => void setAutostart(v)}
          />
          <Row
            label="Closing the window keeps Apex Tune in the tray"
            detail="Quit from the tray icon's menu."
            checked={view.minimizeToTray}
            onChange={(v) => void update({ minimizeToTray: v })}
          />
          <Row
            label="Check for updates when Apex Tune starts"
            detail="Only checks; nothing is downloaded or installed without you."
            checked={view.checkForUpdates}
            onChange={(v) => void update({ checkForUpdates: v })}
          />
        </ul>
        <div className="mt-3 flex flex-wrap items-center gap-3">
          <Button size="sm" onClick={() => void check()} disabled={update_ === "checking"}>
            {update_ === "checking" ? "Checking…" : "Check now"}
          </Button>
          <UpdateMessage result={update_} />
        </div>
      </section>

      <section className="mt-10 max-w-2xl" aria-labelledby="data">
        <h2 id="data" className="text-[19px] font-semibold">
          Logs and data
        </h2>
        <p className="mt-1 text-muted">
          Backups, logs and profiles live in <span className="selectable text-text">{view.dataDir || "the data folder"}</span>.
        </p>
        <div className="mt-3 flex flex-wrap gap-2">
          <Button size="sm" onClick={() => api.openLogsFolder().catch((e) => toast.error(toAppError(e).message))}>
            Open logs folder
          </Button>
          <Button size="sm" variant="ghost" onClick={() => openLog(true)}>
            Show activity log
          </Button>
        </div>
        <p className="mt-6 text-[13px] text-faint">Apex Tune {view.version}</p>
      </section>
    </>
  );
}

function ThemePreview({ t }: { t: (typeof THEMES)[number] }) {
  return (
    <span aria-hidden="true" className="flex h-20 gap-1.5 p-2" style={{ background: t.bg }}>
      <span className="w-6 rounded-sm" style={{ background: t.panel, border: `1px solid ${t.line}` }}>
        <span className="mx-auto mt-1.5 block h-1 w-3 rounded-full" style={{ background: t.signal }} />
      </span>
      <span className="flex flex-1 flex-col gap-1.5 rounded-sm p-1.5" style={{ background: t.panel, border: `1px solid ${t.line}` }}>
        <span className="block h-1.5 w-1/2 rounded-full" style={{ background: t.text, opacity: 0.85 }} />
        <span className="block h-1 w-3/4 rounded-full" style={{ background: t.text, opacity: 0.35 }} />
        <span className="mt-auto block h-3 w-10 rounded-sm" style={{ background: t.signal }} />
      </span>
    </span>
  );
}

function Row({ label, detail, checked, onChange }: { label: string; detail: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <li className="flex items-start gap-4 py-3">
      <div className="min-w-0 flex-1">
        <span className="block">{label}</span>
        <span className="block text-[12px] text-muted">{detail}</span>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
    </li>
  );
}

function UpdateMessage({ result }: { result: UpdateResult | "checking" | null }) {
  if (!result || result === "checking") return null;
  switch (result.kind) {
    case "notConfigured":
      return <span className="text-[13px] text-muted">This build has no update source configured (see the README).</span>;
    case "upToDate":
      return <span className="text-[13px] text-safe">You have the latest version.</span>;
    case "available":
      return (
        <span className="text-[13px] text-signal">
          Version {result.latest} is available{result.url ? ` at ${result.url}` : ""}.
        </span>
      );
    case "failed":
      return <span className="text-[13px] text-moderate">{result.message}</span>;
  }
}
