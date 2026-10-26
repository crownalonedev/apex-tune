/**
 * Typed wrappers for every Tauri command and event. Components never call
 * `invoke` directly; they go through these functions.
 *
 * Outside Tauri (plain `pnpm dev` in a browser, or tests) the calls fall back
 * to clearly-labelled preview data so the UI can be worked on without Windows.
 */
import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---- Types mirrored from src-tauri ----------------------------------------

export interface AppError {
  kind: "io" | "windows" | "other";
  message: string;
}

export interface AppInfo {
  version: string;
  elevated: boolean;
  osName: string;
  osBuild: string;
  dataDir: string;
}

export type ActivityLevel = "info" | "warn" | "error";

export interface ActivityEntry {
  id: number;
  at: string;
  level: ActivityLevel;
  message: string;
  subject: string | null;
}

// ---- Plumbing ---------------------------------------------------------------

export const ACTIVITY_EVENT = "activity://entry";

export const inTauri = (): boolean => isTauri();

function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof (value as AppError).message === "string"
  );
}

/** Normalises anything a command can reject with into an AppError. */
export function toAppError(error: unknown): AppError {
  if (isAppError(error)) return error;
  if (error instanceof Error) return { kind: "other", message: error.message };
  return { kind: "other", message: String(error) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    throw toAppError(error);
  }
}

// ---- Preview data (browser only) ------------------------------------------

const previewInfo: AppInfo = {
  version: "0.1.0",
  elevated: false,
  osName: "Browser preview (not Windows)",
  osBuild: "",
  dataDir: "",
};

// ---- Commands -------------------------------------------------------------

export const api = {
  appInfo(): Promise<AppInfo> {
    return inTauri() ? call<AppInfo>("app_info") : Promise.resolve(previewInfo);
  },

  activityRecent(): Promise<ActivityEntry[]> {
    return inTauri() ? call<ActivityEntry[]>("activity_recent") : Promise.resolve([]);
  },
};

// ---- Events ---------------------------------------------------------------

export async function onActivity(handler: (entry: ActivityEntry) => void): Promise<UnlistenFn> {
  if (!inTauri()) return () => {};
  return listen<ActivityEntry>(ACTIVITY_EVENT, (event) => handler(event.payload));
}
