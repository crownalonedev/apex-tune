import { create } from "zustand";
import { toast } from "sonner";
import { api, toAppError, type MotionPref, type Settings, type SettingsView, type ThemeName } from "@/lib/tauri";

interface SettingsState {
  view: SettingsView | null;
  /** True when the OS asks for less motion. */
  osReducedMotion: boolean;
  load: () => Promise<void>;
  update: (patch: Partial<Settings>) => Promise<void>;
  setAutostart: (enabled: boolean) => Promise<void>;
}

const darkQuery = () => window.matchMedia?.("(prefers-color-scheme: light)");
const motionQuery = () => window.matchMedia?.("(prefers-reduced-motion: reduce)");

/** The concrete theme for a preference. "System" follows Windows' app mode. */
export function resolveTheme(theme: ThemeName): Exclude<ThemeName, "system"> {
  if (theme !== "system") return theme;
  return darkQuery()?.matches ? "daylight" : "graphite";
}

function applyToDocument(settings: Settings) {
  const root = document.documentElement;
  const next = resolveTheme(settings.theme);
  const swap = () => {
    root.dataset.theme = next;
    root.dataset.motion = settings.motion === "system" ? "" : settings.motion;
  };
  const reduced = settings.motion === "reduced" || (settings.motion === "system" && motionQuery()?.matches);
  // Crossfade between themes where the WebView supports view transitions.
  const doc = document as Document & { startViewTransition?: (cb: () => void) => unknown };
  if (root.dataset.theme && root.dataset.theme !== next && doc.startViewTransition && !reduced) {
    doc.startViewTransition(swap);
  } else {
    swap();
  }
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  view: null,
  osReducedMotion: motionQuery()?.matches ?? false,
  load: async () => {
    try {
      const view = await api.settingsGet();
      set({ view });
      applyToDocument(view);
    } catch (e) {
      toast.error(toAppError(e).message);
    }
  },
  update: async (patch) => {
    const current = get().view;
    if (!current) return;
    const next = { ...current, ...patch };
    set({ view: next });
    applyToDocument(next);
    try {
      await api.settingsSet({
        theme: next.theme,
        motion: next.motion,
        minimizeToTray: next.minimizeToTray,
        checkForUpdates: next.checkForUpdates,
      });
    } catch (e) {
      set({ view: current });
      applyToDocument(current);
      toast.error(toAppError(e).message);
    }
  },
  setAutostart: async (enabled) => {
    try {
      await api.settingsSetAutostart(enabled);
      await get().load();
      toast.success(enabled ? "Apex Tune starts with Windows" : "Apex Tune no longer starts with Windows");
    } catch (e) {
      toast.error(toAppError(e).message, { duration: 12_000 });
    }
  },
}));

/** Framer Motion's setting for the current preference. */
export function motionSetting(pref: MotionPref | undefined): "user" | "always" | "never" {
  if (pref === "reduced") return "always";
  if (pref === "full") return "never";
  return "user";
}

/** Keeps "System" theme and motion in step with Windows while the app runs. */
export function watchSystemPreferences() {
  const handler = () => {
    const view = useSettingsStore.getState().view;
    useSettingsStore.setState({ osReducedMotion: motionQuery()?.matches ?? false });
    if (view) applyToDocument(view);
  };
  darkQuery()?.addEventListener?.("change", handler);
  motionQuery()?.addEventListener?.("change", handler);
  return () => {
    darkQuery()?.removeEventListener?.("change", handler);
    motionQuery()?.removeEventListener?.("change", handler);
  };
}
