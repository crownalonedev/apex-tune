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

export type AppErrorKind =
  | "io"
  | "windows"
  | "other"
  | "refused"
  | "notApplicable"
  | "verify"
  | "journal"
  | "noBackup"
  | "restorePointRequired"
  | "rolledBack"
  | "rollbackFailed";

export interface AppError {
  kind: AppErrorKind;
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

export interface RestorePoint {
  sequence: number;
  description: string;
  /** WMI datetime, e.g. "20261026120000.000000-000". */
  created: string;
}

export interface SafetyStatus {
  /** System Protection on the system drive; null if it can't be read. */
  protectionEnabled: boolean | null;
  restorePoint: RestorePoint | null;
  restorePointSkipped: boolean;
  activeChanges: number;
  unreadableBackups: string[];
}

export interface RevertReport {
  reverted: string[];
  failed: { tweak: string; message: string }[];
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

const previewSafety: SafetyStatus = {
  protectionEnabled: null,
  restorePoint: null,
  restorePointSkipped: false,
  activeChanges: 0,
  unreadableBackups: [],
};

const notInPreview = (): Promise<never> =>
  Promise.reject<never>({ kind: "other", message: "Not available in the browser preview." });

// ---- Commands -------------------------------------------------------------

export const api = {
  appInfo(): Promise<AppInfo> {
    return inTauri() ? call<AppInfo>("app_info") : Promise.resolve(previewInfo);
  },

  activityRecent(): Promise<ActivityEntry[]> {
    return inTauri() ? call<ActivityEntry[]>("activity_recent") : Promise.resolve([]);
  },

  safetyStatus(): Promise<SafetyStatus> {
    return inTauri() ? call<SafetyStatus>("safety_status") : Promise.resolve(previewSafety);
  },

  createRestorePoint(): Promise<SafetyStatus> {
    return inTauri() ? call<SafetyStatus>("create_restore_point") : notInPreview();
  },

  enableSystemProtection(): Promise<SafetyStatus> {
    return inTauri() ? call<SafetyStatus>("enable_system_protection") : notInPreview();
  },

  skipRestorePoint(): Promise<SafetyStatus> {
    return inTauri() ? call<SafetyStatus>("skip_restore_point") : notInPreview();
  },

  revertEverything(): Promise<RevertReport> {
    return inTauri() ? call<RevertReport>("revert_everything") : notInPreview();
  },

  listRestorePoints(): Promise<RestorePoint[]> {
    return inTauri() ? call<RestorePoint[]>("list_restore_points") : Promise.resolve([]);
  },

  takeRecoveryMessages(): Promise<string[]> {
    return inTauri() ? call<string[]>("take_recovery_messages") : Promise.resolve([]);
  },
};

// ---- Events ---------------------------------------------------------------

export async function onActivity(handler: (entry: ActivityEntry) => void): Promise<UnlistenFn> {
  if (!inTauri()) return () => {};
  return listen<ActivityEntry>(ACTIVITY_EVENT, (event) => handler(event.payload));
}
