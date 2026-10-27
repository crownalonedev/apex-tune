# Apex Tune

A Windows 11 gaming optimizer where every change can be undone. Apex Tune
backs up each setting before touching it, makes a System Restore point before
the first change of a session, never goes near anti-cheat or game processes,
and never weakens Windows security.

Built with Tauri 2 (Rust) and React 18. Research behind every tweak, including
what paid tools do and what was left out, is in [docs/RESEARCH.md](docs/RESEARCH.md).
The original design is in [docs/PLAN.md](docs/PLAN.md).

## What it does

| Page | What's there |
|---|---|
| **Dashboard** | Hardware summary, live CPU/GPU/memory/process graphs (GPU temperature on NVIDIA), Optimization Score, System Check, and **Optimize for Gaming**: a preview of every change in the Safe preset, applied with per-tweak progress. |
| **Debloat** | Remove preinstalled apps (restorable), switch startup items off the way Task Manager does, disable telemetry tasks and a short list of unneeded services, and suggestion, ad and privacy settings. |
| **Power** | The Apex Tune Gaming power plan (desktop and laptop versions), plus switching between your plans. |
| **Timer** | Current timer resolution, a Sleep(1) benchmark that recommends a value for your PC, and a tray helper that holds it. |
| **NVIDIA** | A global low-latency driver profile, per-game profiles, frame caps, and a full driver-settings backup. |
| **Tweaks** | Gaming, input, visual, network (including a DNS switcher), updates (including pause), per-game settings for Steam/Epic/known games, the Minecraft JVM helper, and storage cleanup. |
| **Profiles & backups** | Built-in and saved profiles (export/import as JSON), the full history of changes with each value before and after, per-change revert, Revert Everything, and restore points. |
| **Settings** | Seven themes, motion level, start with Windows, close to tray, update check, logs folder. |

## Safety model

- **Backups first.** Before any value is written, what was there is saved to
  `%ProgramData%\ApexTune\backups\<session>.json` (flushed to disk, written
  atomically). "Not set" and "key didn't exist" are recorded too, so reverting
  removes what Apex Tune added instead of leaving empty keys behind.
- **All or nothing.** If any part of a change fails, or Windows doesn't keep a
  value, everything that change wrote is put back before the error is shown.
- **Crash recovery.** A change interrupted by a crash or power cut is rolled
  back the next time Apex Tune starts.
- **Restore point** before the first change of each session. Windows normally
  skips a restore point if one was made in the last 24 hours, so Apex Tune
  lifts that limit for a moment and puts it back afterwards.
- **A hard guard** refuses, even from a tampered backup file, to touch:
  - Riot Vanguard, Easy Anti-Cheat and BattlEye services, keys, folders,
    startup entries and executables.
  - Defender, the firewall, UAC, SmartScreen, Secure Boot/VBS/Device Guard, TPM.
  - The switches that turn off Windows Update.
  - Run and Winlogon keys.
  - In Image File Execution Options, anything other than `CpuPriorityClass`
    High or Above Normal.
  - Windows' built-in power plans and tasks outside Apex Tune's own folder.
- **No game process access.** Per-game settings are written where Windows
  applies them at launch (IFEO, compatibility layers, NVIDIA profiles).
- **The data folder is locked down.** `%ProgramData%\ApexTune` is writable
  only by Administrators and SYSTEM, because backups decide what the elevated
  app writes during a revert.
- **Activity log.** Every action and error appears in the in-app Activity Log
  and in `%ProgramData%\ApexTune\logs\` (14 days kept).

## Every tweak and how to revert it

Every tweak below is reverted by switching it off in Apex Tune, from
**Profiles & backups → What changed → Revert**, or with **Revert everything**.
Reverting restores the exact previous value, not a "default".

Risk: **S** Safe, **M** Moderate (confirms first), **A** Advanced (confirms first).
★ = part of the Safe "Optimize for Gaming" preset.

### Gaming

| Tweak | Risk | What changes |
|---|---|---|
| Game Mode ★ | S | `HKCU\Software\Microsoft\GameBar` `AutoGameModeEnabled`=1, `AllowAutoGameMode`=1 |
| Turn off background recording ★ | S | `HKCU\System\GameConfigStore` `GameDVR_Enabled`=0; `HKCU\…\GameDVR` `AppCaptureEnabled`=0 |
| Optimizations for windowed games ★ | S | Adds `SwapEffectUpgradeEnable=1;` to `HKCU\Software\Microsoft\DirectX\UserGpuPreferences` `DirectXUserGlobalSettings`, keeping the other entries |
| Xbox button doesn't open Game Bar | S | `HKCU\Software\Microsoft\GameBar` `UseNexusForGameBarEnabled`=0 |
| Hardware-accelerated GPU scheduling | M | `HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers` `HwSchMode`=2 (restart) |
| Less CPU reserved for background tasks | M | `…\Multimedia\SystemProfile` `SystemResponsiveness`=10 (restart) |
| Higher scheduling priority for games that ask for it | M | `…\SystemProfile\Tasks\Games`: GPU Priority 8, Priority 6, Scheduling Category High, SFIO Priority High (restart) |
| Apex Tune Gaming power plan ★ | S | New plan copied from Ultimate Performance (fallback High performance, then Balanced) with the CPU at 100%, core parking off, USB/PCIe power saving off; made active. Revert reactivates your previous plan and deletes the new one. |
| System-wide timer requests | A | `HKLM\…\Session Manager\kernel` `GlobalTimerResolutionRequests`=1 (Windows 11 only, restart) |
| NVIDIA low-latency defaults ★ | S | Global driver profile: Low Latency Mode on, Prefer maximum performance, high-performance texture filtering, threaded optimization, 10 GB shader cache. Revert puts back your values or the driver defaults. NVIDIA only. |

### Input

| Tweak | Risk | What changes |
|---|---|---|
| Turn off Enhance Pointer Precision ★ | S | `HKCU\Control Panel\Mouse` `MouseSpeed`/`MouseThreshold1`/`MouseThreshold2`="0", applied live |
| No Sticky Keys pop-up from Shift ★ | S | `HKCU\Control Panel\Accessibility` StickyKeys `Flags`="506", ToggleKeys "58", Keyboard Response "122", applied live |

### Network

| Tweak | Risk | What changes |
|---|---|---|
| No network throttling while audio plays | M | `…\Multimedia\SystemProfile` `NetworkThrottlingIndex`=0xFFFFFFFF (restart) |
| Turn off Ethernet power saving | M | Wired adapter properties `*EEE`, `EnableGreenEthernet`, `AdvancedEEE` = "0", where present (restart) |
| Acknowledge TCP packets immediately | A | `Tcpip\Parameters\Interfaces\{adapter}` `TcpAckFrequency`=1 (restart). TCP only; little effect on UDP games. |
| DNS switcher | – | Per adapter: Cloudflare, Google, Quad9 or Automatic. Revert restores the adapter's previous servers. |

### Visual

| Tweak | Risk | What changes |
|---|---|---|
| Turn off transparency | S | `HKCU\…\Themes\Personalize` `EnableTransparency`=0 |
| Turn off window animations | S | Window and minimise animations off (SystemParametersInfo), `TaskbarAnimations`=0 |
| Adjust for best performance (keep smooth fonts) | S | Menu, tooltip, list, shadow and drag effects off through SystemParametersInfo; font smoothing kept on; `ListviewAlphaSelect`/`ListviewShadow`/`EnableAeroPeek`=0 |

### Suggestions and privacy

| Tweak | Risk | What changes |
|---|---|---|
| No Start menu suggestions ★ | S | `ContentDeliveryManager` suggestion values = 0; `Start_IrisRecommendations`, `Start_AccountNotifications` = 0 |
| No tips and "finish setup" prompts ★ | S | Tips, welcome experience, Settings suggestions = 0; `ScoobeSystemSettingEnabled`=0 |
| No lock screen ads ★ | S | `RotatingLockScreenOverlayEnabled`=0, `SubscribedContent-338387Enabled`=0 |
| No File Explorer promotions ★ | S | `ShowSyncProviderNotifications`=0 |
| No advertising ID ★ | S | `AdvertisingInfo` `Enabled`=0; `TailoredExperiencesWithDiagnosticDataEnabled`=0 |
| Turn off Widgets ★ | S | Policy `HKLM\SOFTWARE\Policies\Microsoft\Dsh` `AllowNewsAndInterests`=0 |
| Stop Edge running in the background ★ | S | Edge policies `StartupBoostEnabled`=0, `BackgroundModeEnabled`=0 |
| Stop Store apps running in the background | M | `BackgroundAccessApplications` `GlobalUserDisabled`=1 |
| No web results in Start search | M | `HKCU\Software\Policies\Microsoft\Windows\Explorer` `DisableSearchBoxSuggestions`=1 |
| Send only required diagnostic data | S | Policy `DataCollection` `AllowTelemetry`=1 |
| Turn off Recall snapshots | S | `HKCU\…\WindowsAI` `DisableAIDataAnalysis`=1 |

### Updates and system

| Tweak | Risk | What changes |
|---|---|---|
| Windows Update leaves drivers alone | M | Policy `ExcludeWUDriversInQualityUpdate`=1 |
| Don't upload updates to other PCs | S | Policy `DeliveryOptimization` `DODownloadMode`=0 |
| Delay yearly feature updates by 180 days | M | Policy `DeferFeatureUpdates`=1, `DeferFeatureUpdatesPeriodInDays`=180 (Pro and above) |
| Turn off Fast Startup | M | `Session Manager\Power` `HiberbootEnabled`=0 (restart) |
| Pause Windows Update | – | The same values Settings → Windows Update writes, for 1–5 weeks. "Resume now" reverts them. |

### Per-item actions (each revertible on its own)

- **Apps.** Removed for your account only; Windows keeps its copy, so
  "Bring back" restores it offline. The reviewed list never includes the
  Store, App Installer, Gaming Services, the Xbox app, Xbox sign-in, Game Bar,
  Windows Security or Minecraft.
- **Startup items.** Switched off through `StartupApproved`, like Task Manager;
  the entry itself is kept. Third-party sign-in tasks are disabled, not deleted.
- **Telemetry tasks and services.** Each one is changed and restored on its own.
  Some tasks are owned by Windows and refuse; those are reported and left alone.
- **Per-game High CPU priority.** IFEO `PerfOptions\CpuPriorityClass`=3 for that
  executable.
- **Per-game fullscreen optimizations off.** Adds `DISABLEDXMAXIMIZEDWINDOWEDMODE`
  to the game's compatibility flags, keeping any existing flags.
- **NVIDIA per-game profile and frame cap.** Written to the game's driver
  profile; a profile Apex Tune had to create is deleted on revert. NVIDIA
  settings also have a whole-database backup with one-click restore.
- **Refresh rate fix** (System Check). Switches a monitor to its highest
  refresh rate; revert puts the old rate back.
- **Timer helper sign-in task** and **Start with Windows** are tasks in Task
  Scheduler's `\ApexTune` folder.

### One-way actions (can't be reverted)

Storage cleanup deletes cached files, which can't be put back. Each target
shows its size first and asks before deleting, and files in use are skipped.
The targets are temp folders, the Windows Update download cache, the Delivery
Optimization cache, GPU shader caches and old error reports.

## Building

### Requirements (on Windows 11)

- [Rust](https://rustup.rs) stable, MSVC toolchain
- Visual Studio 2022 Build Tools with "Desktop development with C++"
- Node.js 20+ and [pnpm](https://pnpm.io)
- WebView2 runtime (preinstalled on Windows 11)

### Run in development

Apex Tune requires administrator rights, so Windows refuses to start it from
a normal terminal (error 740). Open **Terminal (Admin)**, then:

```powershell
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` builds the `apex-timer` tray helper first (`pnpm sidecar`).

The UI also runs in a normal browser with `pnpm dev` at
http://localhost:1420. There it shows labelled preview data and never touches
the system.

### Build the installers

```powershell
pnpm tauri build
```

This produces an MSI and an NSIS installer in
`src-tauri\target\release\bundle\`. Both install per-machine. Uninstalling
removes Apex Tune's own scheduled tasks and stops the timer helper, but
**doesn't revert tweaks**: use Revert everything first. The NSIS uninstaller
reminds you.

**Code signing.** The bundle is ready to sign: set
`bundle.windows.certificateThumbprint` in `src-tauri/tauri.conf.json` to your
certificate's thumbprint (SHA-256 digest and DigiCert timestamping are
already configured), or use Tauri's `signCommand` for cloud signing. Unsigned
builds work but show a SmartScreen warning.

**Update checks.** Set `VITE_UPDATE_URL` at build time to a GitHub
"latest release" API URL (for example
`https://api.github.com/repos/<owner>/<repo>/releases/latest`). Without it,
Settings says no update source is configured. Apex Tune only checks; it never
downloads anything.

### Tests and checks

```powershell
pnpm test                 # Vitest (UI)
pnpm typecheck            # TypeScript
cd src-tauri
cargo test --workspace    # Rust unit tests (no admin needed)
cargo clippy --workspace --all-targets -- -D warnings
```

The Rust tests run every tweak's detect/apply/revert, rollback on failure,
crash recovery, the guard and the journal against in-memory fakes of the
registry, services, power plans, tasks, apps, displays, network and NVAPI. So
they run on any OS, including macOS and Linux, without changing anything.

## Where things are stored

`%ProgramData%\ApexTune\`

- `backups\`: one JSON journal per session, plus `nvidia-*.nvdrs` driver-settings backups
- `logs\`: daily log files, 14 days kept
- `profiles\`: saved profiles
- `settings.json`, `timer.json`: preferences

## Known limitations

- **Not yet run on real hardware.** Development happened on macOS. All
  Windows code compiles and passes `clippy` for `x86_64-pc-windows-msvc`, and
  the logic is unit-tested against fakes, but the Windows API calls (registry,
  SCM, WMI, powrprof, Task Scheduler, PackageManager, NVAPI, display modes,
  DNS) need a first run on a Windows PC to confirm.
- **NVAPI function IDs** come from the open-source nvapi-rs bindings. NVIDIA
  doesn't document them publicly; a future driver could change them, in which
  case the NVIDIA page reports no NVIDIA support rather than misbehaving.
- **"Ultra" Low Latency Mode** isn't in NVIDIA's public settings header, so
  Apex Tune sets "On" (one pre-rendered frame) only.
- **System Protection detection** reads the volume list Windows keeps under
  `SPP\Clients`; if it's misread, "Turn on System Protection" is harmless to
  click.
- **CPU temperature** isn't shown: Windows only exposes it through a kernel
  driver, which anti-cheat may flag.
- **Elevating with a different admin account.** Per-user settings go to the
  signed-in user's registry hive. App removal and SystemParametersInfo
  effects still apply to the elevated account in that case.
- **Apps you installed yourself** (not preinstalled) may need reinstalling
  from the Store instead of "Bring back", because Windows doesn't keep a copy.
- **Feature-update deferral** is a Group Policy setting; Windows Home ignores
  it, and the tweak says so.
- **Timer resolution** mostly helps games and frame limiters that pace with
  `Sleep()`. The System-wide requests switch is needed on Windows 11 for the
  helper to affect games.
- **Deliberately left out**, with reasons in [docs/RESEARCH.md](docs/RESEARCH.md):
  - disabling CPU security mitigations, VBS/Memory Integrity, Defender,
    SmartScreen, UAC or Windows Update
  - bcdedit timer hacks, `Win32PrioritySeparation`, forced MSI mode
  - memory compression and SysMain changes
  - "RAM boosters" and live process priority or affinity changes
