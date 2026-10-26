# Apex Tune: architecture plan and tweak catalog

Status: **draft, waiting for approval**. Nothing has been scaffolded yet.

---

## 1. Architecture

```
apex-tune/
├─ src-tauri/                    Rust backend (all system work)
│  ├─ src/
│  │  ├─ main.rs                 Tauri setup, tray, single-instance, command registration
│  │  ├─ commands/               Thin #[tauri::command] wrappers (no logic)
│  │  ├─ sys/                    The ONLY place that touches Windows
│  │  │  ├─ mod.rs               traits: Registry, Services, Power, Tasks, Appx, Spi, Fs
│  │  │  ├─ win/                 real impls (#[cfg(windows)], windows-rs)
│  │  │  └─ fake.rs              in-memory impls for unit tests (run on any OS)
│  │  ├─ tweaks/                 Tweak trait + one module per category
│  │  │  ├─ mod.rs               trait, registry of all tweaks, preset definitions
│  │  │  ├─ primitives.rs        RegValue / ServiceStart / SpiFlag / TaskState "atoms"
│  │  │  ├─ gaming.rs input.rs network.rs visual.rs debloat.rs updates.rs
│  │  ├─ backup/                 journal, restore points, Revert Everything
│  │  ├─ power/                  plan duplication, settings, laptop variant
│  │  ├─ timer/                  NtQuery/NtSetTimerResolution, benchmark
│  │  ├─ nvidia/                 NVAPI FFI (hand-written, dynamically loaded nvapi64.dll), DRS profiles
│  │  ├─ storage/                cleaners (one-way actions, see 3.9)
│  │  ├─ hw/                     sysinfo + WMI detection, live stats stream
│  │  ├─ guard.rs                anti-cheat/security deny-list, checked before EVERY mutation
│  │  └─ log.rs                  tracing → rolling file + UI event channel
│  └─ apex-timer/                separate tiny no-webview binary (tray) that holds the timer resolution
└─ src/                          React 18 + TS + Vite + Tailwind + shadcn/ui
   ├─ pages/  components/  stores/ (Zustand)  lib/tauri.ts (typed command wrappers)
```

### The `Tweak` trait

```rust
pub trait Tweak: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;   // plain English, includes honest effect size
    fn category(&self) -> Category;
    fn risk(&self) -> Risk;                   // Safe | Moderate | Advanced
    fn requires_restart(&self) -> Restart;    // None | SignOut | Explorer | Reboot
    fn applicable(&self, sys: &SysCtx) -> Applicability; // e.g. "Pro only", "no NVIDIA GPU"
    fn detect(&self, sys: &SysCtx) -> Result<TweakState>; // Applied | NotApplied | Partial | Unknown
    fn plan(&self, sys: &SysCtx) -> Result<Vec<Change>>;  // exact changes, used for the preview
    fn apply(&self, sys: &SysCtx, tx: &mut Txn) -> Result<()>;
    fn revert(&self, sys: &SysCtx, tx: &mut Txn) -> Result<()>;
}
```

Most tweaks are declarative: a list of atoms (`RegValue { hive, path, name, value }`, `ServiceStart { name, start }`, `SpiFlag`, `ScheduledTask`, …). Each atom knows how to read, write and restore itself, so `detect/apply/revert` for 90% of tweaks is generic code that gets tested once. Only the odd ones (DNS, AppX, power, NVIDIA, timer) are hand-written.

### Transactions and backups (safety rules 1, 2, 6, 7)

1. **Write-ahead journal.** Before any atom is written, its original value (including "value did not exist" and "key did not exist") is appended to `%ProgramData%\ApexTune\backups\<session-id>.json` and flushed (temp file + atomic rename).
2. **Apply** writes atom by atom, then calls `detect()` to verify. If any write or the verification fails, the transaction rolls back the atoms already written in reverse order, marks the tweak as failed, and reports the error. A tweak is never left half-applied.
3. **Revert** restores from the journal entry (not from "Windows defaults"), so it puts back exactly what was there, including things a previous tool had changed.
4. **Restore point** once per session before the first mutation (WMI `SystemRestore.CreateRestorePoint`). On a fresh reset System Protection is often **off** on C:; the app detects this and asks to turn it on. Windows throttles restore points to one per 24 h; the app temporarily sets `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore\SystemRestorePointCreationFrequency = 0` and puts the old value back afterwards (journaled like any other change).
5. **Revert Everything** walks all session journals newest-first.
6. **Guard.** Every mutation passes through `guard.rs`, which hard-refuses: services/drivers `vgc`, `vgk`, `EasyAntiCheat*`, `BEService`, `BEDaisy`, `WinDefend`, `WdNisSvc`, `Sense`, `SecurityHealthService`, `wscsvc`, `mpssvc`, `BFE`, `wuauserv`, `UsoSvc`, `BITS`, `TrustedInstaller`, `GamingServices*`, `XblAuthManager`, `XboxGipSvc`, `NVDisplay.ContainerLocalSystem`; files/dirs under `Riot Vanguard`, `EasyAntiCheat`, `BattlEye`; registry under `Policies\Microsoft\Windows Defender`, UAC (`EnableLUA`), SmartScreen, firewall, Secure Boot/HVCI/VBS keys; IFEO `Debugger` values. No code path opens a handle to a game process.
7. **Logging.** `tracing` → `%ProgramData%\ApexTune\logs\apex-tune.log` (daily rotation, 14 days) and an event channel the UI's Activity Log subscribes to.

### Elevation and autostart

- The main exe manifest is `requireAdministrator`.
- Elevated apps can't autostart from Run keys (UAC silently blocks them), so "Start with Windows" uses a **Task Scheduler logon task with "Run with highest privileges"**.
- The timer tray helper (`apex-timer.exe`) does not need admin and starts from its own logon task.

### Testing

- `sys::fake` gives in-memory registry/services/power/tasks, so every tweak's `detect/apply/revert`, rollback-on-failure, journal round-trip and guard refusal is unit-tested on any OS.
- Vitest + Testing Library for stores, `lib/tauri.ts` (mocked IPC) and key components.
- **Limitation:** I'm on your Mac, not your gaming PC. I can run the unit tests and Vitest here, and run `cargo clippy --target x86_64-pc-windows-msvc` via `cargo-xwin` to compile-check the Windows code. **I can't run the app or apply any real tweak here.** Each stage ends with steps for you to run it on the PC, and the real Windows integration tests (`cargo test --features win-integration`, run as admin) only run there.

---

## 2. Presets

| Preset | Contents |
|---|---|
| **Optimize for Gaming (Safe)** | Every tweak marked ★ below. Safe only, no reboot-only surprises without saying so. |
| Competitive | Safe + Moderate items you tick. Never selected automatically. |
| Custom | Whatever you toggle; can be saved as a profile. |

The Optimization Score is the weighted share of ★ and recommended Moderate tweaks that `detect()` reports as Applied (weights: Safe 1, Moderate 0.5; Advanced doesn't count, so you never "lose points" for skipping them).

---

## 3. Tweak catalog

Legend: **Risk** S = Safe, M = Moderate, A = Advanced. **Restart** — = none, X = Explorer restart, O = sign out, R = reboot. ★ = in the Safe gaming preset. All HKCU values are written for the interactive user (the elevated process resolves the logged-on user's hive, not the admin's, if they differ).

### 3.1 Gaming

| ID | Change | Risk | Restart | Notes |
|---|---|---|---|---|
| `game_mode` ★ | `HKCU\Software\Microsoft\GameBar` `AutoGameModeEnabled` DWORD 1, `AllowAutoGameMode` DWORD 1 | S | — | On by default in Win11; this makes sure. |
| `hags` | `HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers` `HwSchMode` DWORD 2 (on) / 1 (off) | M | R | Required for DLSS Frame Generation. Effect on latency varies by game; shown as a toggle, not in the preset. |
| `game_dvr_off` ★ | `HKCU\System\GameConfigStore` `GameDVR_Enabled` DWORD 0; `HKCU\Software\Microsoft\Windows\CurrentVersion\GameDVR` `AppCaptureEnabled` DWORD 0 | S | O | Stops background recording. Game Bar itself keeps working (no policy key used). |
| `windowed_opt` ★ | `HKCU\Software\Microsoft\DirectX\UserGpuPreferences` `DirectXUserGlobalSettings` REG_SZ, merge `SwapEffectUpgradeEnable=1;` | S | — | Win11 "Optimizations for windowed games": flip-model presentation for DX10/11 borderless games (Valorant, Fortnite DX11). Lower latency, and needed for VRR in windowed mode. The string is parsed and merged, not overwritten. |
| `fso_per_game` | `HKCU\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers` value = full exe path, data `~ DISABLEDXMAXIMIZEDWINDOWEDMODE` (existing flags kept) | A | — | Per game only. On Win11 FSO usually *helps*, so this is opt-in for a game where you measured a problem. |
| `mmcss_responsiveness` | `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile` `SystemResponsiveness` DWORD 10 (default 20) | M | R | Documented minimum is 10; 0 is treated as 10, so the app writes 10, not the folklore 0. Only affects MMCSS-registered threads (audio, some games). |
| `mmcss_games_task` | `...\SystemProfile\Tasks\Games`: `GPU Priority` DWORD 8, `Priority` DWORD 6, `Scheduling Category` SZ `High`, `SFIO Priority` SZ `High` | M | R | Only affects games that register with MMCSS as "Games"; few do. Labelled "small/variable effect". |
| `ifeo_priority` | `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\<exe>\PerfOptions` `CpuPriorityClass` DWORD 3 (High) | M | — (next launch) | Per exe you choose. Never sets `Debugger` or anything else under IFEO. Revert deletes only the `PerfOptions` key it created. Presets for `VALORANT-Win64-Shipping.exe`, `FortniteClient-Win64-Shipping.exe`, `javaw.exe` (applies to **all** Java apps, warned), `Minecraft.Windows.exe`. |

### 3.2 Input

| ID | Change | Risk | Restart | Notes |
|---|---|---|---|---|
| `mouse_accel_off` ★ | `SystemParametersInfo(SPI_SETMOUSE, [0,0,0], SPIF_UPDATEINIFILE\|SPIF_SENDCHANGE)` → persists `HKCU\Control Panel\Mouse` `MouseSpeed`="0", `MouseThreshold1`="0", `MouseThreshold2`="0" | S | — | Applied live, no sign-out. |
| `input_queue_size` | `HKLM\SYSTEM\CurrentControlSet\Services\mouclass\Parameters` `MouseDataQueueSize`, `...\kbdclass\Parameters` `KeyboardDataQueueSize` (default 100) | A | R | **See decision D1.** This is a buffer size, not a latency setting; I can't find evidence that lowering it helps, and too low drops input. |

### 3.3 Network

| ID | Change | Risk | Restart | Notes |
|---|---|---|---|---|
| `net_throttling_off` | `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile` `NetworkThrottlingIndex` DWORD `0xFFFFFFFF` (default 10) | M | R | Stops MMCSS throttling non-multimedia packets while audio plays. Real but small. |
| `tcp_ack_frequency` | `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{adapter-GUID}` `TcpAckFrequency` DWORD 1 | A | R | **See decision D2.** TCP only. Valorant and Fortnite gameplay is UDP, so no effect there; Minecraft Java already sets `TCP_NODELAY` itself. `TCPNoDelay` is not honored by modern Windows' TCP stack and won't be written. |
| `dns` | `SetInterfaceDnsSettings` (iphlpapi) per active adapter: Cloudflare `1.1.1.1, 1.0.0.1` / `2606:4700:4700::1111, ::1001`; Google `8.8.8.8, 8.8.4.4` / `2001:4860:4860::8888, ::8844`; Automatic (DHCP) | S | — | Backup stores each adapter's previous static/DHCP config. Honest note: affects lookup speed, not in-game ping. |

### 3.4 Visual

| ID | Change | Risk | Restart | Notes |
|---|---|---|---|---|
| `transparency_off` | `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` `EnableTransparency` DWORD 0 | S | — | |
| `animations_off` | `SPI_SETCLIENTAREAANIMATION` false; `HKCU\Control Panel\Desktop\WindowMetrics` `MinAnimate`="0"; `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced` `TaskbarAnimations` DWORD 0 | S | X | |
| `best_performance_visuals` | `HKCU\...\Explorer\VisualEffects` `VisualFXSetting` DWORD 3 (Custom) + individual SPI calls: menu/combobox/listbox/tooltip/selection-fade animations off, drop shadows off, cursor shadow off, `DragFullWindows` off, `HKCU\Software\Microsoft\Windows\DWM` `EnableAeroPeek` 0, `ListviewAlphaSelect`/`ListviewShadow` 0; **`SPI_SETFONTSMOOTHING` stays on** | S | O | Uses SPI calls instead of hand-editing the `UserPreferencesMask` blob. Honest note: makes the desktop snappier; no measurable FPS change in fullscreen games. |

### 3.5 Debloat: consumer suggestions and privacy

All under `HKCU\Software\Microsoft\Windows\CurrentVersion\` unless shown.

| ID | Change | Risk | Restart |
|---|---|---|---|
| `start_suggestions_off` ★ | `ContentDeliveryManager`: `SystemPaneSuggestionsEnabled` 0, `SubscribedContent-338388Enabled` 0, `SilentInstalledAppsEnabled` 0; `Explorer\Advanced` `Start_IrisRecommendations` 0, `Start_AccountNotifications` 0 | S | X |
| `tips_off` ★ | `ContentDeliveryManager`: `SubscribedContent-338389Enabled` 0, `SoftLandingEnabled` 0, `SubscribedContent-310093Enabled` 0, `SubscribedContent-338393Enabled` 0, `SubscribedContent-353694Enabled` 0, `SubscribedContent-353696Enabled` 0; `UserProfileEngagement` `ScoobeSystemSettingEnabled` 0 | S | — |
| `lockscreen_ads_off` ★ | `ContentDeliveryManager`: `RotatingLockScreenOverlayEnabled` 0, `SubscribedContent-338387Enabled` 0 (Spotlight itself is left alone) | S | — |
| `explorer_ads_off` ★ | `Explorer\Advanced` `ShowSyncProviderNotifications` 0 | S | X |
| `ad_id_off` ★ | `AdvertisingInfo` `Enabled` 0; `Privacy` `TailoredExperiencesWithDiagnosticDataEnabled` 0 | S | — |
| `web_search_off` | `HKCU\Software\Policies\Microsoft\Windows\Explorer` `DisableSearchBoxSuggestions` DWORD 1 | M | X |
| `telemetry_required` | `HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection` `AllowTelemetry` DWORD 1 (Required). On Home/Pro, 0 is treated as 1, so 1 is the honest value. | S | — |
| `diagtrack_off` | Service `DiagTrack` start → Disabled, stopped | M | — |

### 3.6 Debloat: scheduled tasks (disabled, not deleted)

Via the Task Scheduler COM API. Tasks owned by TrustedInstaller that refuse are skipped and reported, not forced. Names vary between 23H2/24H2/25H2, so missing tasks show as "Not present".

- `\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser` (and `... Appraiser Exp`), `ProgramDataUpdater`, `PcaPatchDbTask`
- `\Microsoft\Windows\Customer Experience Improvement Program\Consolidator`, `UsbCeip`
- `\Microsoft\Windows\Autochk\Proxy`
- `\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector`
- `\Microsoft\Windows\Feedback\Siuf\DmClient`, `DmClientOnScenarioDownload`
- `\Microsoft\Windows\Maps\MapsUpdateTask`, `MapsToastTask`

### 3.7 Debloat: services (conservative)

Backup stores start type **and** the DelayedAutostart flag.

| Service | Default | Recommended | Risk |
|---|---|---|---|
| `Fax` (Pro only) | Manual | Disabled | S |
| `RetailDemo` | Manual | Disabled | S |
| `MapsBroker` | Automatic (Delayed) | Manual | S |
| `DiagTrack` | Automatic | Disabled (same as `diagtrack_off`) | M |
| `Spooler` | Automatic | Disabled, **only if you never print** (no printers detected → offered; otherwise hidden) | M |
| `wisvc` (Insider) | Manual | Disabled | S |

Everything else is shown read-only. `SysMain`, `WSearch` and `TabletInputService` are deliberately left alone (disabling them either does nothing for games or breaks search, the emoji panel and the touch keyboard).

### 3.8 Debloat: AppX packages

Removal uses `Windows.Management.Deployment.PackageManager` (WinRT via windows-rs), **current user only** by default. The provisioned copy stays on disk, so **revert = re-register** (`RegisterPackageByFamilyNameAsync`), which works offline. "Also remove for new users (deprovision)" is an Advanced option, and its revert is "reinstall from the Store" (stated before you confirm).

**Safe to remove (checked by default only for ones you tick):** `Clipchamp.Clipchamp`, `Microsoft.BingNews`, `Microsoft.BingWeather`, `Microsoft.MicrosoftSolitaireCollection`, `MSTeams` / `MicrosoftTeams` (consumer Teams), `Microsoft.GetHelp`, `Microsoft.Getstarted` (Tips), `Microsoft.MicrosoftOfficeHub`, `Microsoft.PowerAutomateDesktop`, `Microsoft.Todos`, `Microsoft.WindowsFeedbackHub`, `Microsoft.WindowsMaps`, `Microsoft.ZuneVideo`, `Microsoft.WindowsAlarms`, `Microsoft.OutlookForWindows`, `Microsoft.Copilot`, `Microsoft.549981C3F5F10` (Cortana, if present), `Microsoft.People` (if present).

**Optional (unchecked, with a note):** `Microsoft.ZuneMusic` (Media Player), `Microsoft.YourPhone` (Phone Link), `Microsoft.MicrosoftStickyNotes`.

**Keep (never removable in the UI):** `Microsoft.WindowsStore`, `Microsoft.StorePurchaseApp`, `Microsoft.DesktopAppInstaller` (winget), `Microsoft.GamingServices`, `Microsoft.GamingApp` (Xbox app, needed for Game Pass and the Bedrock launcher path), `Microsoft.XboxIdentityProvider` and `Microsoft.Xbox.TCUI` (Xbox sign-in for Bedrock), `Microsoft.XboxGamingOverlay` (Game Bar), `Microsoft.XboxSpeechToTextOverlay`, `Microsoft.SecHealthUI`, all framework packages (`VCLibs`, `UI.Xaml`, `NET.Native.*`, `WindowsAppRuntime`), codecs (`HEIF`, `HEVC`, `VP9`, `WebMedia`, `WebpImage`, `RawImage`, `AV1`), `NVIDIACorp.NVIDIAControlPanel`, Minecraft packages, and anything marked NonRemovable. Unknown packages show read-only.

**Note:** "Xbox extras" turn out to be almost all dependencies for Bedrock sign-in or Game Bar, so the removable Xbox list is empty on purpose.

### 3.9 Startup apps

- **Run keys** (`HKCU`/`HKLM\...\Run`, plus `WOW6432Node`) and the **Startup folders** are disabled the same way Task Manager does it: `...\Explorer\StartupApproved\Run` (or `Run32` / `StartupFolder`) binary value `03 00 00 00 …` = disabled, `02 …` = enabled. The original entry is never deleted, so revert is exact.
- **Logon/boot scheduled tasks** from third parties are disabled via the Task Scheduler API.
- Protected (shown, not toggleable): `vgtray` / Riot Vanguard, EAC, BattlEye, `SecurityHealth`, NVIDIA driver entries.

### 3.10 Windows Update (no disabling)

| ID | Change | Risk |
|---|---|---|
| `pause_updates` | Writes the same values Settings → Pause does: `HKLM\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings` `PauseUpdatesStartTime`, `PauseUpdatesExpiryTime`, `PauseFeatureUpdates*`, `PauseQualityUpdates*` (ISO-8601), 1–5 weeks | S |
| `defer_feature_updates` | `HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate` `DeferFeatureUpdates` 1, `DeferFeatureUpdatesPeriodInDays` 0–365. **Pro/Enterprise only;** shown as not applicable on Home. Security (quality) updates are never deferred. | M |

### 3.11 Storage cleanup (one-way actions)

Deleting caches can't be "reverted", so these are **actions, not tweaks**: no toggle, a confirm dialog showing the size before deletion, reclaimed space after, and a log entry. Files in use are skipped.

- `%TEMP%`, `C:\Windows\Temp` (files older than 24 h)
- Windows Update download cache `C:\Windows\SoftwareDistribution\Download` (stops `wuauserv` + `BITS`, cleans, **always restarts them**, even on error)
- Delivery Optimization cache (via the DoSvc API path; `DoSvc` stopped/restarted)
- Shader caches `%LOCALAPPDATA%\NVIDIA\DXCache`, `GLCache`, `%LOCALAPPDATA%\D3DSCache` (warning: expect stutter for the first few matches while it rebuilds; skipped while a game is running)

---

## 4. Power plan

Built with `powrprof` (`PowerDuplicateScheme`, `PowerWriteACValueIndex`/`DCValueIndex`, `PowerSetActiveScheme`), not by shelling out to `powercfg`. The source is Ultimate Performance `e9a42b02-d5df-448d-aa00-03f14749eb61`; if it's hidden (common on laptops), the app duplicates High Performance `8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c` instead.

| Setting | Subgroup / setting GUID | Desktop (AC) | Laptop variant |
|---|---|---|---|
| Min processor state | `54533251-…-b60b740d00` / `893dee8e-2bef-41e0-89c6-b55d0929964c` | 100% | AC 100%, DC 5% |
| Core parking (min cores) | `54533251-…` / `0cc5b647-c1df-4637-891a-dec35c318583` | 100% | AC 100%, DC default |
| USB selective suspend | `2a737441-1930-4402-8d77-b2bebba308a3` / `48e6b7a6-50f5-4782-a5d4-53bb8f07e226` | Disabled | AC disabled, DC enabled |
| PCIe Link State PM | `501a4d13-42af-4429-9fd1-a8218c268e20` / `ee12f906-d277-404b-b6da-e5fa1a576df5` | Off | AC off, DC moderate |
| Hard disk off | `0012ee47-9041-4b5d-9b77-535fba8b1442` / `6738e2c4-e8a5-4a42-b16a-e040e769756e` | Never | AC never, DC 10 min |
| Display off | `7516b95f-f776-4464-8c53-06167f40cc99` / `3c0bc021-c8a8-4e07-a973-6b14cbcb2b7e` | 15 min | AC 15, DC 5 min |
| Sleep | `238c9fa8-0aad-41ed-83f4-97be242c8f20` / `29f6c1db-86da-48c5-9fdb-f2b67b1f44da` | Never | AC 60 min, DC 15 min |

Honest note: 100% minimum state stops idle down-clocking, so idle power and temperatures go up. That's fine on a desktop, and that's why the laptop variant keeps DC sane. Revert = switch back to the previously active plan and delete the Apex plan. Also included: list all plans, switch between them, and "Restore Balanced" (`381b4222-f694-41f0-9685-ff5bb260df2e`).

---

## 5. Timer resolution

- Read via `NtQueryTimerResolution` (min/max/current, shown in ms) and set via `NtSetTimerResolution` from `ntdll`.
- **Win11 caveat:** since Win10 2004 the resolution is per-process, and Win11 also ignores requests from processes whose windows are minimized or hidden. Without the global flag, `apex-timer.exe` only affects itself. So:
  - `global_timer_requests` (A, R): `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\kernel` `GlobalTimerResolutionRequests` DWORD 1. The timer page tells you plainly that the setting does nothing without this flag.
- `apex-timer.exe`: a tray icon, ~2 MB RAM, holds the requested resolution, and releases it on exit.
- **Benchmark:** for each candidate (0.5, 0.504, 0.75, 1.0 ms…, clamped to the machine's max) it runs N×`Sleep(1)` timed with `QueryPerformanceCounter` and reports mean/p99 deviation from 1 ms. It recommends the value with the lowest p99 deviation, not just the smallest number (0.5 ms often oversleeps more than ~0.504 ms on some machines).
- Honest note: this mainly helps games or frame limiters that pace with `Sleep()`. Optional start with Windows via a logon task.

---

## 6. NVIDIA profile (NVAPI DRS)

- `nvapi64.dll` is loaded dynamically through `nvapi_QueryInterface` with hand-written FFI (`nvapi-sys` is unmaintained). If there's no DLL or no NVIDIA GPU, the page shows a clear "No NVIDIA GPU detected" state and everything else keeps working.
- **Backup:** before any change, `NvAPI_DRS_SaveSettingsToFileEx` exports the **entire driver settings database** to the backup folder, and per-setting original values are journaled too. "Full restore" = `NvAPI_DRS_LoadSettingsFromFileEx`.
- **Profiles:** the global "Apex Tune Low Latency" changes are applied to the *base/global* profile. For games, the driver already ships application profiles (an exe can belong to only one profile), so the app **edits the existing profile** for `VALORANT-Win64-Shipping.exe`, `FortniteClient-Win64-Shipping.exe`, `javaw.exe`, `Minecraft.Windows.exe` and picked Steam exes, and creates one only if none exists.

| Setting | NVAPI constant | Value |
|---|---|---|
| Low Latency Mode | `PRERENDERLIMIT_ID` (+ the Ultra CPL state setting) | On (1) / Ultra |
| Power management | `PREFERRED_PSTATE_ID` | Prefer maximum performance |
| Texture filtering quality | `QUALITY_ENHANCEMENTS_ID` | High performance |
| Threaded optimization | `OGL_THREAD_CONTROL_ID` | On (relevant for Minecraft Java/OpenGL) |
| Shader cache size | `PS_SHADERDISKCACHE_MAX_SIZE` | Unlimited / 10 GB |
| Max frame rate | `FRL_FPS_ID` | user value, off by default |
| Vertical sync | `VSYNCMODE_ID` | Off globally; G-SYNC users get a "VSync on + cap 3 below refresh" hint |

Exact setting IDs and values are taken from `NvApiDriverSettings.h` (R5xx SDK) during implementation and covered by a unit test that pins them.

**Reflex reminder:** Valorant and Fortnite have NVIDIA Reflex. When Reflex is on, the driver's Low Latency Mode is ignored, so for those two the app sets LLM off and shows "Turn on Reflex: On + Boost in-game". LLM is applied to Minecraft and non-Reflex Steam games.

---

## 7. Steam and Minecraft extras

- Steam: read the install path from `HKCU\Software\Valve\Steam\SteamPath`, parse `steamapps\libraryfolders.vdf` and each `appmanifest_*.acf` to list installed games and their exes for profile targeting.
- Minecraft Java JVM helper (generates text you paste into the launcher; it doesn't edit launcher files):
  - RAM recommendation: ≤8 GB installed → 3 GB; 16 GB → 4–6 GB; ≥32 GB → 6–8 GB (vanilla/light mods vs heavy modpacks), never more than half of RAM.
  - Flags: modern G1GC set (Aikar-style flags for Java 17/21), with a note that over-allocating RAM makes GC pauses worse.

---

## 8. UI

- Pages: Dashboard, Debloat, Power, Timer, NVIDIA, Tweaks, Profiles & Backups, Settings, plus an Activity Log drawer available everywhere.
- Every tweak card shows the title, plain-English description including an honest effect size, a risk badge, a restart badge, the detected current state (Applied / Not applied / Partial / Not applicable), and a toggle. Moderate and Advanced toggles open a confirmation dialog listing the exact changes from `plan()`.
- "Optimize for Gaming" opens a preview diff (registry path / old → new) before anything runs, then shows per-tweak progress and a "Restart needed" banner if any applied tweak requires one.
- Dark theme by default with one restrained accent; skeleton loaders, toasts, Framer Motion page/list transitions (respecting `prefers-reduced-motion`), full keyboard navigation, visible focus rings.
- Dashboard live stats stream from Rust once per second (sysinfo for CPU/RAM/processes, NVAPI for GPU usage/temperature, WMI `MSAcpi_ThermalZoneTemperature` for CPU temperature where exposed, which is often not the case on desktops; shown as "n/a" rather than a fake number).

---

## 9. Build order (unchanged from your brief)

scaffold → `Tweak` trait + backup/journal + guard → Dashboard → Power → Timer → Debloat → Custom Tweaks → NVIDIA → Profiles → Settings → installer (Tauri bundler: MSI + NSIS, `requireAdministrator` manifest, unsigned but ready for a code-signing cert) → README.

After each stage: `cargo test`, `cargo clippy --all-targets -- -D warnings` (host and Windows target), `pnpm vitest run`, `pnpm tsc --noEmit`, plus "how to run it on your PC" steps.

---

## 10. Decisions I need from you

- **D1 — Input data-queue size.** I recommend showing the current value read-only with a "reset to default (100)" button, not a tuning slider, because I can't find evidence that it lowers latency (it conflicts with your no-placebo rule). Alternatively: keep it as an Advanced toggle as you specified.
- **D2 — Nagle / `TcpAckFrequency`.** I recommend keeping only `TcpAckFrequency` as Advanced, with the honest "TCP only, your shooters use UDP" label, and dropping `TCPNoDelay` (ignored by modern Windows). Alternatively: drop the Nagle tweak entirely.
- **D3 — AppX removal scope.** Default is current user only, with an exact offline revert. OK?
- **D4 — Storage cleaners** are one-way actions outside the backup system (they can't be reverted by nature). OK?
- **D5 — Repo.** This folder currently sits inside a git repo rooted at your home directory. I'd `git init` a separate repo in `~/Documents/Optimizer` at the scaffold stage.
