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
  gpuTempC: number | null;
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

export interface Game {
  name: string;
  exe: string;
  path: string | null;
  source: "known" | "steam" | "epic";
  note: string | null;
  highPriority: boolean;
  fsoDisabled: boolean;
}

export type Collector = "g1" | "zgc";

export interface JvmAdvice {
  memoryGb: number;
  arguments: string;
  notes: string[];
}

export type DnsPreset = "automatic" | "cloudflare" | "google" | "quad9";

export interface AdapterDns {
  guid: string;
  name: string;
  description: string;
  wired: boolean;
  connected: boolean;
  servers: string[];
  preset: DnsPreset | null;
}

export interface PauseStatus {
  until: string | null;
  ours: boolean;
}

export type CleanTarget = "userTemp" | "windowsTemp" | "updateCache" | "deliveryOptimization" | "shaderCache" | "errorReports";

export interface CleanTargetInfo {
  target: CleanTarget;
  name: string;
  description: string;
  warning: string | null;
  bytes: number;
  files: number;
}

export interface CleanReport {
  freedBytes: number;
  deletedFiles: number;
  skippedFiles: number;
}

export type FindingLevel = "good" | "info" | "warn";
export type Fix = { kind: "refreshRate"; device: string; hz: number };

export interface Finding {
  id: string;
  title: string;
  detail: string;
  level: FindingLevel;
  fix: Fix | null;
}

export interface NvidiaStatus {
  info: { gpu: string; driver: string } | null;
  games: { name: string; exe: string; reflex: boolean; applied: boolean; fpsCap: number | null }[];
  hasBackup: boolean;
}

export interface ProfileView {
  format: string;
  version: number;
  id: string;
  name: string;
  description: string;
  createdAt: string;
  tweaks: string[];
  builtIn: boolean;
  applied: number;
}

export interface ChangeView {
  target: string;
  before: string;
  after: string;
}

export interface EntryView {
  session: string;
  entry: string;
  name: string;
  action: "apply" | "revert" | "system";
  status: "inProgress" | "done" | "rolledBack" | "rollbackFailed";
  at: string;
  error: string | null;
  revertible: boolean;
  reverted: boolean;
  changes: ChangeView[];
}

export interface SessionView {
  id: string;
  startedAt: string;
  restorePoint: RestorePoint | null;
  restorePointSkipped: boolean;
  entries: EntryView[];
}

export type ThemeName = "system" | "graphite" | "ember" | "moss" | "violet" | "oled" | "daylight";
export type MotionPref = "system" | "full" | "reduced";

export interface Settings {
  theme: ThemeName;
  motion: MotionPref;
  minimizeToTray: boolean;
  checkForUpdates: boolean;
}

export interface SettingsView extends Settings {
  startWithWindows: boolean;
  dataDir: string;
  version: string;
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

const previewGames: Game[] = [
  { name: "VALORANT", exe: "VALORANT-Win64-Shipping.exe", path: null, source: "known", note: "Uses NVIDIA Reflex: turn on Reflex + Boost in its video settings.", highPriority: false, fsoDisabled: false },
  { name: "Minecraft: Java Edition", exe: "javaw.exe", path: null, source: "known", note: "javaw.exe is shared by every Java app.", highPriority: false, fsoDisabled: false },
  { name: "Counter-Strike 2", exe: "cs2.exe", path: "D:\\SteamLibrary\\steamapps\\common\\Counter-Strike Global Offensive\\game\\bin\\win64\\cs2.exe", source: "steam", note: null, highPriority: false, fsoDisabled: false },
];

const previewDns: AdapterDns[] = [
  { guid: "{PREVIEW}", name: "Ethernet", description: "Preview adapter", wired: true, connected: true, servers: [], preset: "automatic" },
];

const previewStorage: CleanTargetInfo[] = [
  { target: "userTemp", name: "Your temporary files", description: "Leftovers from installers and apps.", warning: null, bytes: 1.4 * GB, files: 3120 },
  { target: "shaderCache", name: "GPU shader caches", description: "Compiled shaders from NVIDIA, AMD and DirectX.", warning: "Games stutter for the first few minutes while shaders are rebuilt.", bytes: 2.1 * GB, files: 812 },
];

const previewFindings: Finding[] = [
  { id: "refresh", title: "Preview monitor runs at 60 Hz but supports 165 Hz", detail: "Windows often leaves high-refresh monitors at 60 Hz.", level: "warn", fix: { kind: "refreshRate", device: "\\\\.\\DISPLAY1", hz: 165 } },
  { id: "secure_boot", title: "Secure Boot is on", detail: "Riot Vanguard needs it on Windows 11.", level: "good", fix: null },
];

const previewNvidia: NvidiaStatus = {
  info: { gpu: "Preview NVIDIA GPU", driver: "560.94" },
  games: [
    { name: "VALORANT", exe: "VALORANT-Win64-Shipping.exe", reflex: true, applied: false, fpsCap: null },
    { name: "Minecraft: Java Edition", exe: "javaw.exe", reflex: false, applied: false, fpsCap: null },
  ],
  hasBackup: false,
};

const previewProfiles: ProfileView[] = [
  { format: "apex-tune-profile", version: 1, id: "builtin.gaming", name: "Gaming (Safe preset)", description: "The Optimize for Gaming preset.", createdAt: "2026-10-26T12:00:00Z", tweaks: ["game_mode", "game_dvr_off", "windowed_games"], builtIn: true, applied: 1 },
  { format: "apex-tune-profile", version: 1, id: "builtin.competitive", name: "Competitive", description: "The Safe preset plus Moderate tweaks for lower latency and a lighter desktop.", createdAt: "2026-10-26T12:00:00Z", tweaks: ["game_mode", "game_dvr_off", "windowed_games", "mmcss_responsiveness"], builtIn: true, applied: 1 },
];

function previewSettings(): SettingsView {
  let saved: Partial<Settings> = {};
  try {
    saved = JSON.parse(localStorage.getItem("apex-preview-settings") ?? "{}") as Partial<Settings>;
  } catch {
    // Ignore unreadable preview settings.
  }
  return {
    theme: "graphite",
    motion: "system",
    minimizeToTray: false,
    checkForUpdates: true,
    ...saved,
    startWithWindows: false,
    dataDir: "",
    version: "0.1.0",
  };
}

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

  gamesList(): Promise<Game[]> {
    return inTauri() ? call<Game[]>("games_list") : Promise.resolve(previewGames);
  },

  gameSetPriority(exe: string, on: boolean): Promise<void> {
    return inTauri() ? call<void>("game_set_priority", { exe, on }) : notInPreview();
  },

  gameSetFso(path: string, on: boolean): Promise<void> {
    return inTauri() ? call<void>("game_set_fso", { path, on }) : notInPreview();
  },

  minecraftJvm(modded: boolean, collector: Collector): Promise<JvmAdvice> {
    return inTauri()
      ? call<JvmAdvice>("minecraft_jvm", { modded, collector })
      : Promise.resolve({
          memoryGb: modded ? 8 : 4,
          arguments: `-Xms${modded ? 8 : 4}G -Xmx${modded ? 8 : 4}G ${collector === "zgc" ? "-XX:+UseZGC -XX:+ZGenerational" : "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200"}`,
          notes: ["Preview values."],
        });
  },

  dnsStatus(): Promise<AdapterDns[]> {
    return inTauri() ? call<AdapterDns[]>("dns_status") : Promise.resolve(previewDns);
  },

  dnsSet(guid: string | null, preset: DnsPreset): Promise<AdapterDns[]> {
    return inTauri() ? call<AdapterDns[]>("dns_set", { guid, preset }) : notInPreview();
  },

  updatesPauseStatus(): Promise<PauseStatus> {
    return inTauri() ? call<PauseStatus>("updates_pause_status") : Promise.resolve({ until: null, ours: false });
  },

  updatesPause(weeks: number): Promise<PauseStatus> {
    return inTauri() ? call<PauseStatus>("updates_pause", { weeks }) : notInPreview();
  },

  updatesResume(): Promise<PauseStatus> {
    return inTauri() ? call<PauseStatus>("updates_resume") : notInPreview();
  },

  storageScan(): Promise<CleanTargetInfo[]> {
    return inTauri() ? call<CleanTargetInfo[]>("storage_scan") : Promise.resolve(previewStorage);
  },

  storageClean(target: CleanTarget): Promise<CleanReport> {
    return inTauri() ? call<CleanReport>("storage_clean", { target }) : notInPreview();
  },

  insightsRun(): Promise<Finding[]> {
    return inTauri() ? call<Finding[]>("insights_run") : Promise.resolve(previewFindings);
  },

  insightsFix(fix: Fix): Promise<void> {
    return inTauri() ? call<void>("insights_fix", { fix }) : notInPreview();
  },

  nvidiaStatus(): Promise<NvidiaStatus> {
    return inTauri() ? call<NvidiaStatus>("nvidia_status") : Promise.resolve(previewNvidia);
  },

  nvidiaSetGame(exe: string, on: boolean): Promise<NvidiaStatus> {
    return inTauri() ? call<NvidiaStatus>("nvidia_set_game", { exe, on }) : notInPreview();
  },

  nvidiaSetFps(exe: string, fps: number): Promise<NvidiaStatus> {
    return inTauri() ? call<NvidiaStatus>("nvidia_set_fps", { exe, fps }) : notInPreview();
  },

  nvidiaRestoreBackup(): Promise<void> {
    return inTauri() ? call<void>("nvidia_restore_backup") : notInPreview();
  },

  backupsSessions(): Promise<SessionView[]> {
    return inTauri() ? call<SessionView[]>("backups_sessions") : Promise.resolve([]);
  },

  backupsRevertEntry(session: string, entry: string): Promise<void> {
    return inTauri() ? call<void>("backups_revert_entry", { session, entry }) : notInPreview();
  },

  openSystemRestore(): Promise<void> {
    return inTauri() ? call<void>("open_system_restore") : notInPreview();
  },

  profilesList(): Promise<ProfileView[]> {
    return inTauri() ? call<ProfileView[]>("profiles_list") : Promise.resolve(previewProfiles);
  },

  profileSave(name: string): Promise<ProfileView> {
    return inTauri() ? call<ProfileView>("profile_save", { name }) : notInPreview();
  },

  profileApply(id: string, exclusive: boolean): Promise<BatchReport> {
    return inTauri() ? call<BatchReport>("profile_apply", { id, exclusive }) : notInPreview();
  },

  profileDelete(id: string): Promise<void> {
    return inTauri() ? call<void>("profile_delete", { id }) : notInPreview();
  },

  profileExport(id: string, path: string): Promise<void> {
    return inTauri() ? call<void>("profile_export", { id, path }) : notInPreview();
  },

  profileImport(path: string): Promise<{ profile: ProfileView; dropped: string[] }> {
    return inTauri() ? call("profile_import", { path }) : notInPreview();
  },

  settingsGet(): Promise<SettingsView> {
    return inTauri() ? call<SettingsView>("settings_get") : Promise.resolve(previewSettings());
  },

  settingsSet(settings: Settings): Promise<void> {
    if (inTauri()) return call<void>("settings_set", { settings });
    try {
      localStorage.setItem("apex-preview-settings", JSON.stringify(settings));
    } catch {
      // Preview only; nothing to persist to.
    }
    return Promise.resolve();
  },

  settingsSetAutostart(enabled: boolean): Promise<void> {
    return inTauri() ? call<void>("settings_set_autostart", { enabled }) : notInPreview();
  },

  openLogsFolder(): Promise<void> {
    return inTauri() ? call<void>("open_logs_folder") : notInPreview();
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
      gpuTempC: Math.round(58 + 6 * Math.sin(t / 11)),
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
