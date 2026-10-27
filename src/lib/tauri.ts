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

export type GpuVendor = "nvidia" | "amd" | "intel" | "other";
export type DriveKind = "nvmeSsd" | "ssd" | "hdd" | "unknown";

export interface HardwareSummary {
  cpu: { name: string; physicalCores: number | null; logicalCores: number; baseMhz: number };
  memoryBytes: number;
  gpus: {
    name: string;
    vendor: GpuVendor;
    driverVersion: string;
    nvidiaDriver: string | null;
    vramBytes: number | null;
  }[];
  drives: { name: string; kind: DriveKind; sizeBytes: number }[];
  volumes: { mount: string; totalBytes: number; freeBytes: number }[];
  windows: {
    productName: string;
    displayVersion: string | null;
    build: number;
    revision: number | null;
    edition: string | null;
  } | null;
  hasBattery: boolean;
  warnings: string[];
}

export interface StatsTick {
  at: number;
  cpuPercent: number;
  ramUsedBytes: number;
  ramTotalBytes: number;
  processes: number;
  gpuPercent: number | null;
}

export type Risk = "safe" | "moderate" | "advanced";
export type Restart = "none" | "explorer" | "signOut" | "reboot";
export type Category =
  | "gaming"
  | "input"
  | "network"
  | "visual"
  | "privacy"
  | "startup"
  | "services"
  | "updates"
  | "power"
  | "timer"
  | "nvidia"
  | "system";

export interface TweakMeta {
  id: string;
  name: string;
  description: string;
  category: Category;
  risk: Risk;
  restart: Restart;
  recommended: boolean;
}

export type TweakState =
  | { state: "applied" }
  | { state: "notApplied" }
  | { state: "partial" }
  | { state: "notApplicable"; reason: string }
  | { state: "unknown"; reason: string };

export interface TweakInfo extends TweakMeta {
  state: TweakState;
  revertible: boolean;
}

export interface Score {
  value: number | null;
  counted: number;
  applied: number;
}

export interface Overview {
  tweaks: TweakInfo[];
  score: Score;
}

export interface ChangePreview {
  target: string;
  from: string;
  to: string;
}

export interface PresetItem extends TweakMeta {
  state: TweakState;
  changes: ChangePreview[];
}

export type BatchStatus = "applied" | "alreadyApplied" | "failed";

export interface BatchResult {
  id: string;
  name: string;
  status: BatchStatus;
  errorKind: AppErrorKind | null;
  error: string | null;
}

export interface BatchReport {
  results: BatchResult[];
  restart: Restart;
}

export interface PlanInfo {
  guid: string;
  name: string;
  active: boolean;
  builtIn: boolean;
  apex: boolean;
}

export interface PowerSettingRow {
  label: string;
  targetAc: string;
  targetDc: string | null;
  currentAc: string | null;
  currentDc: string | null;
}

export interface PowerOverview {
  plans: PlanInfo[];
  laptop: boolean;
  balancedInstalled: boolean;
  settings: PowerSettingRow[];
}

export interface TimerStatus {
  resolution: { coarsest: number; finest: number; current: number } | null;
  helperAvailable: boolean;
  helperRunning: boolean;
  /** 100 ns units. */
  units: number;
  autostart: boolean;
}

export interface BenchRow {
  requested: number | null;
  granted: number | null;
  meanMs: number;
  p99Ms: number;
  maxMs: number;
  samples: number;
}

export interface BenchReport {
  rows: BenchRow[];
  recommended: number | null;
}

export type AppGroup = "removable" | "optional" | "keep" | "other";

export interface AppRow {
  name: string;
  displayName: string;
  group: AppGroup;
  note: string;
  installed: boolean;
  restorable: boolean;
}

export type StartupSource = "userRun" | "machineRun" | "machineRun32" | "userFolder" | "commonFolder" | "task";

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  source: StartupSource;
  enabled: boolean;
  protected: string | null;
}

export type StartType = "boot" | "system" | "automatic" | "automaticDelayed" | "manual" | "disabled";

export interface TaskRow {
  path: string;
  name: string;
  description: string;
  enabled: boolean | null;
}

export interface ServiceRow {
  name: string;
  display: string;
  description: string;
  current: StartType | null;
  running: boolean;
  recommended: StartType;
  risk: Risk;
}

export interface Background {
  tasks: TaskRow[];
  services: ServiceRow[];
  changed: string[];
}

export interface ItemResult {
  id: string;
  ok: boolean;
  error: string | null;
}

export const APEX_PLAN_TWEAK = "power_plan";
export const BALANCED_PLAN = "381b4222-f694-41f0-9685-ff5bb260df2e";

// ---- Plumbing ---------------------------------------------------------------

export const ACTIVITY_EVENT = "activity://entry";
export const STATS_EVENT = "stats://tick";
export const PRESET_PROGRESS_EVENT = "preset://progress";
export const BENCH_PROGRESS_EVENT = "timer://bench";

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

const GB = 1024 ** 3;

const previewHardware: HardwareSummary = {
  cpu: { name: "Preview CPU (8 cores)", physicalCores: 8, logicalCores: 16, baseMhz: 4200 },
  memoryBytes: 32 * GB,
  gpus: [
    {
      name: "Preview NVIDIA GPU",
      vendor: "nvidia",
      driverVersion: "32.0.15.6094",
      nvidiaDriver: "560.94",
      vramBytes: 12 * GB,
    },
  ],
  drives: [{ name: "Preview NVMe drive", kind: "nvmeSsd", sizeBytes: 1000 * 1000 ** 3 }],
  volumes: [{ mount: "C:\\", totalBytes: 930 * GB, freeBytes: 512 * GB }],
  windows: null,
  hasBattery: false,
  warnings: [],
};

const previewOverview: Overview = {
  tweaks: [],
  score: { value: null, counted: 0, applied: 0 },
};

const previewPower: PowerOverview = {
  plans: [
    { guid: BALANCED_PLAN, name: "Balanced", active: true, builtIn: true, apex: false },
    { guid: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", name: "High performance", active: false, builtIn: true, apex: false },
  ],
  laptop: false,
  balancedInstalled: true,
  settings: [
    { label: "Minimum processor state", targetAc: "100%", targetDc: null, currentAc: null, currentDc: null },
    { label: "Core parking (minimum unparked cores)", targetAc: "100%", targetDc: null, currentAc: null, currentDc: null },
    { label: "USB selective suspend", targetAc: "Off", targetDc: null, currentAc: null, currentDc: null },
    { label: "PCI Express Link State Power Management", targetAc: "Off", targetDc: null, currentAc: null, currentDc: null },
    { label: "Turn off hard disk after", targetAc: "Never", targetDc: null, currentAc: null, currentDc: null },
    { label: "Turn off display after", targetAc: "15 min", targetDc: null, currentAc: null, currentDc: null },
    { label: "Sleep after", targetAc: "Never", targetDc: null, currentAc: null, currentDc: null },
  ],
};

const previewTimer: TimerStatus = {
  resolution: { coarsest: 15.625, finest: 0.5, current: 1 },
  helperAvailable: true,
  helperRunning: false,
  units: 5000,
  autostart: false,
};

const previewApps: AppRow[] = [
  { name: "Clipchamp.Clipchamp", displayName: "Clipchamp", group: "removable", note: "Video editor.", installed: true, restorable: false },
  { name: "Microsoft.BingNews", displayName: "News", group: "removable", note: "News app; also feeds Widgets.", installed: true, restorable: false },
  { name: "Microsoft.YourPhone", displayName: "Phone Link", group: "optional", note: "Keep it if you use your phone with this PC.", installed: true, restorable: false },
  { name: "Microsoft.GamingServices", displayName: "Gaming Services", group: "keep", note: "Needed by Game Pass and Minecraft.", installed: true, restorable: false },
];

const previewStartup: StartupItem[] = [
  { id: "userRun:Discord", name: "Discord", command: "Update.exe --processStart Discord.exe", source: "userRun", enabled: true, protected: null },
  { id: "machineRun:SecurityHealth", name: "SecurityHealth", command: "SecurityHealthSystray.exe", source: "machineRun", enabled: true, protected: "Windows Security." },
];

const previewBackground: Background = {
  tasks: [
    {
      path: "\\Microsoft\\Windows\\Application Experience\\Microsoft Compatibility Appraiser",
      name: "Microsoft Compatibility Appraiser",
      description: "Scans installed programs and sends compatibility telemetry.",
      enabled: true,
    },
  ],
  services: [
    {
      name: "DiagTrack",
      display: "Connected User Experiences and Telemetry",
      description: "Sends diagnostic data to Microsoft in the background.",
      current: "automatic",
      running: true,
      recommended: "disabled",
      risk: "moderate",
    },
  ],
  changed: [],
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

  hardwareSummary(refresh = false): Promise<HardwareSummary> {
    return inTauri()
      ? call<HardwareSummary>("hardware_summary", { refresh })
      : Promise.resolve(previewHardware);
  },

  statsSubscribe(): Promise<void> {
    return inTauri() ? call<void>("stats_subscribe") : Promise.resolve();
  },

  statsUnsubscribe(): Promise<void> {
    return inTauri() ? call<void>("stats_unsubscribe") : Promise.resolve();
  },

  tweaksOverview(): Promise<Overview> {
    return inTauri() ? call<Overview>("tweaks_overview") : Promise.resolve(previewOverview);
  },

  applyTweak(id: string): Promise<"applied" | "alreadyApplied"> {
    return inTauri() ? call("apply_tweak", { id }) : notInPreview();
  },

  revertTweak(id: string): Promise<void> {
    return inTauri() ? call<void>("revert_tweak", { id }) : notInPreview();
  },

  presetPreview(): Promise<PresetItem[]> {
    return inTauri() ? call<PresetItem[]>("preset_preview") : Promise.resolve([]);
  },

  powerOverview(): Promise<PowerOverview> {
    return inTauri() ? call<PowerOverview>("power_overview") : Promise.resolve(previewPower);
  },

  powerSwitch(guid: string): Promise<PowerOverview> {
    return inTauri() ? call<PowerOverview>("power_switch", { guid }) : notInPreview();
  },

  tweakPlan(id: string): Promise<ChangePreview[]> {
    return inTauri() ? call<ChangePreview[]>("tweak_plan", { id }) : Promise.resolve([]);
  },

  timerStatus(): Promise<TimerStatus> {
    return inTauri() ? call<TimerStatus>("timer_status") : Promise.resolve(previewTimer);
  },

  timerStart(units: number): Promise<TimerStatus> {
    return inTauri() ? call<TimerStatus>("timer_start", { units }) : notInPreview();
  },

  timerStop(): Promise<TimerStatus> {
    return inTauri() ? call<TimerStatus>("timer_stop") : notInPreview();
  },

  timerSetAutostart(enabled: boolean): Promise<TimerStatus> {
    return inTauri() ? call<TimerStatus>("timer_set_autostart", { enabled }) : notInPreview();
  },

  timerBenchmark(): Promise<BenchReport> {
    return inTauri() ? call<BenchReport>("timer_benchmark") : notInPreview();
  },

  debloatApps(): Promise<AppRow[]> {
    return inTauri() ? call<AppRow[]>("debloat_apps") : Promise.resolve(previewApps);
  },

  debloatRemoveApps(names: string[]): Promise<ItemResult[]> {
    return inTauri() ? call<ItemResult[]>("debloat_remove_apps", { names }) : notInPreview();
  },

  debloatRestoreApp(name: string): Promise<void> {
    return inTauri() ? call<void>("debloat_restore_app", { name }) : notInPreview();
  },

  debloatStartup(): Promise<StartupItem[]> {
    return inTauri() ? call<StartupItem[]>("debloat_startup") : Promise.resolve(previewStartup);
  },

  debloatSetStartup(id: string, enabled: boolean): Promise<void> {
    return inTauri() ? call<void>("debloat_set_startup", { id, enabled }) : notInPreview();
  },

  debloatBackground(): Promise<Background> {
    return inTauri() ? call<Background>("debloat_background") : Promise.resolve(previewBackground);
  },

  debloatApplyBackground(tasks: string[], services: string[]): Promise<ItemResult[]> {
    return inTauri() ? call<ItemResult[]>("debloat_apply_background", { tasks, services }) : notInPreview();
  },

  debloatRestoreBackground(key: string): Promise<void> {
    return inTauri() ? call<void>("debloat_restore_background", { key }) : notInPreview();
  },

  applyPreset(ids: string[]): Promise<BatchReport> {
    return inTauri() ? call<BatchReport>("apply_preset", { ids }) : notInPreview();
  },
};

// ---- Events ---------------------------------------------------------------

/** Live stats. Outside Tauri, emits synthetic preview ticks. */
export async function onStats(handler: (tick: StatsTick) => void): Promise<UnlistenFn> {
  if (inTauri()) return listen<StatsTick>(STATS_EVENT, (event) => handler(event.payload));
  let t = 0;
  const timer = setInterval(() => {
    t += 1;
    handler({
      at: Date.now(),
      cpuPercent: 18 + 10 * Math.sin(t / 5) + Math.random() * 6,
      ramUsedBytes: (11 + Math.sin(t / 9)) * GB,
      ramTotalBytes: 32 * GB,
      processes: 212,
      gpuPercent: 40 + 25 * Math.sin(t / 7) + Math.random() * 8,
    });
  }, 1000);
  return () => clearInterval(timer);
}

export async function onPresetProgress(handler: (result: BatchResult) => void): Promise<UnlistenFn> {
  if (!inTauri()) return () => {};
  return listen<BatchResult>(PRESET_PROGRESS_EVENT, (event) => handler(event.payload));
}

export async function onActivity(handler: (entry: ActivityEntry) => void): Promise<UnlistenFn> {
  if (!inTauri()) return () => {};
  return listen<ActivityEntry>(ACTIVITY_EVENT, (event) => handler(event.payload));
}

export async function onBenchProgress(handler: (done: number, total: number) => void): Promise<UnlistenFn> {
  if (!inTauri()) return () => {};
  return listen<[number, number]>(BENCH_PROGRESS_EVENT, (event) => handler(...event.payload));
}
