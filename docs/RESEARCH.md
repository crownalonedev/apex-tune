# Research notes: what Apex Tune includes, and what it leaves out

Apex Tune's rules: every change is reversible, has a documented real effect, never touches anti-cheat
or game processes, and never weakens Windows security. This file records what popular tools offer,
what the evidence says, and what Apex Tune does with each.

Last researched: October 2026.

## What the paid tools do

| Tool | What it offers | What Apex Tune takes from it |
|---|---|---|
| **Hone** (free + Premium) | Debloat, TCP/IP tweaks, "Optimize FPS & Input Lag", max-performance power mode, timer resolution, per-game presets, and "Disable Mitigations" in Premium. Saves originals and makes a restore point. [hone.gg support](https://support.hone.gg/hc/en-gb/articles/4758238924959-Recommended-Optimizations-to-Use), [blog](https://hone.gg/blog/the-new-hone-optimizer/) | The reversible-preset model, a power plan, timer resolution, and debloat. **Not** "Disable Mitigations": it turns off CPU security mitigations (Spectre/Meltdown), which is security sabotage. |
| **Process Lasso** (Bitsum) | ProBalance (lowers priority of background hogs), CPU Sets, Performance Mode (its own high-performance plan), automatic power-plan switching. [bitsum.com](https://bitsum.com/), [FAQ](https://bitsum.com/pl-faq/) | The power plan. **Not** live priority or affinity changes on running processes: that needs a handle to the game process, which anti-cheat may treat as tampering. Apex Tune uses only the documented IFEO `CpuPriorityClass`, which Windows applies at launch. |
| **Razer Cortex** | Closes "non-essential" processes and "frees RAM" while a game runs, then restores them. Independent reviews say it does little. [razer.com](https://www.razer.com/ca-en/cortex/boost), [Softpedia](https://games.softpedia.com/get/Tools/Razer-Cortex.shtml) | Nothing. Killing processes risks closing launchers and anti-cheat. "Freeing RAM" empties Windows' cache, which then has to be reloaded. Apex Tune removes background load permanently and reversibly (startup items, Widgets, Edge background mode). |
| **Winhance / Chris Titus WinUtil** (free) | Large registry tweak collections. [winhance docs](https://winhance.net/docs/features/optimizations/gaming-performance.html) | Used to cross-check registry paths, e.g. `SwapEffectUpgradeEnable` for windowed games. |

## Findings that changed the catalog

- **Optimizations for windowed games** move DX10/11 borderless games to the flip presentation model, which lowers latency and enables VRR and Auto HDR in windowed mode ([Microsoft](https://support.microsoft.com/en-us/topic/3f006843-2c7e-4ed0-9a5e-f9389e535952)). Some games show tearing ([gHacks](https://www.ghacks.net/?p=177157)), so the value is merged into `DirectXUserGlobalSettings` and can be reverted. **In, Safe, in the preset.**
- **Hardware-accelerated GPU scheduling** shows no consistent FPS gain in tests, but it's required for DLSS Frame Generation ([PCWorld](https://www.pcworld.com/article/2339130/should-you-enable-hardware-accelerated-gpu-scheduling-in-windows-11.html)). **In, Moderate, not in the preset.**
- **VBS / Memory Integrity** costs 0–6% in most games (worst cases ~10% on older CPUs) ([PCWorld](https://www.pcworld.com/article/541137/tested-does-windows-11-nuke-gaming-performance.html), [Digital Trends](https://digitaltrends.com/computing/are-windows-11-security-features-killing-gaming-performance)). Turning it off weakens kernel protection. **Shown in System Check as information only; Apex Tune never changes it.**
- **Valorant on Windows 11** needs TPM 2.0 and Secure Boot (errors VAN 9001/9003) ([esports.net](https://www.esports.net/wiki/guides/how-to-install-riot-vanguard/)). **System Check shows both states so you know Valorant will launch.**
- **Win32PrioritySeparation**: the popular `0x26` value is short, variable quanta with maximum foreground boost, which is what the client default `2` already means. Other values only have forum anecdotes behind them ([Blur Busters](https://forums.blurbusters.com/viewtopic.php?p=94787)). **Out (placebo at best).**
- **bcdedit `useplatformclock` / `disabledynamictick`**: no controlled tests, and reports of stutter that persists until reinstall ([TechPowerUp](https://techpowerup.com/forums/goto/post?id=5324791), [Blur Busters](https://forums.blurbusters.com/viewtopic.php?p=67665)). **Out.**
- **MSI mode for the GPU**: recent NVIDIA drivers enable it on RTX 30/40, the driver resets it on install, and results are mixed ([Guru3D](https://forums.guru3d.com/goto/post?id=6366171), [igor'sLAB](https://www.igorslab.de/en/interrupt-problems-read-message-signaled-interrupts-msi/1/)). **System Check reports whether the GPU uses MSI; Apex Tune doesn't change it.**
- **Memory compression off / SysMain off**: no gaming benchmark shows a gain, and disabling compression can raise disk use ([ElevenForum](https://www.elevenforum.com/t/enable-or-disable-memory-compression-in-windows-10-and-windows-11.3555/)). **Out.**
- **Ethernet Energy Efficient Ethernet / Green Ethernet**: some drivers drop to low power mid-game and cause stutter ([Steam](https://steamcommunity.com/app/353380/discussions/0/405690850599636269), [Intel tuning guide](https://edc.intel.com/content/www/br/pt/design/products/ethernet/appnote-perf-tuning-guide-700-800-series-windows/​interrupt-moderation)). Uses the standard `*EEE` adapter keyword. **In, Moderate, wired adapters only.** Interrupt moderation off raises CPU use; **out**.
- **Background apps off** (`GlobalUserDisabled`) stops Store apps running in the background but breaks Windows Spotlight wallpapers ([winhelponline](https://www.winhelponline.com/blog/cant-set-windows-spotlight-background-windows-11/)). **In, Moderate.**
- **Game Bar controller button** (`UseNexusForGameBarEnabled`) opens Game Bar over your game when you press the Xbox button ([ElevenForum](https://elevenforum.com/t/enable-or-disable-open-game-bar-using-xbox-button-on-controller-in-windows-11.4290/latest)). **In, Safe, optional.**
- **Game DVR policy** (`AllowGameDVR`) also disables Game Bar captures, which is too broad ([DISA STIG](https://www.stigviewer.com/stigs/microsoft_windows_11_v2r7/2026-02-12/finding/V-253399)). **Apex Tune uses the per-user values instead.**
- **Monitor refresh rate**: Windows often leaves high-refresh monitors at 60 Hz ([Blur Busters](https://www.blurbusters.com/oh-no-im-at-the-wrong-refresh-rate/), [PCWorld](https://www.pcworld.com/article/3620954/how-to-change-your-monitors-refresh-rate-and-why-gamers-should-check-pronto)). **System Check detects it and offers a one-click, reversible switch.**
- **RAM below rated speed (XMP/EXPO off)**: `Win32_PhysicalMemory.ConfiguredClockSpeed` well below `Speed` suggests the kit is running at JEDEC defaults ([Microsoft](https://learn.microsoft.com/nl-nl/windows/win32/cimwin32prov/win32-physicalmemory), [CachyOS forum](https://discuss.cachyos.org/t/memory-speed-issue-with-cachyos/22129)). **System Check shows it as a BIOS tip; Windows can't change it.**
- **Minecraft Java**: Aikar-style G1 flags suit 4–12 GB heaps; generational ZGC (Java 21+) has tiny pauses but uses more memory ([mineguard](https://mineguard.pro/en/blog/zgc-vs-g1gc-minecraft-java-21-benchmark)). **The JVM helper offers both, G1 by default.**

## NVIDIA driver settings (from NVIDIA's own NVAPI headers)

Source: [NVIDIA/nvapi `NvApiDriverSettings.h`](https://github.com/NVIDIA/nvapi/blob/main/NvApiDriverSettings.h).

| Setting | ID | Values used |
|---|---|---|
| Maximum pre-rendered frames (Low Latency Mode) | `0x007BA09E` | 1 = On, 0 = app-controlled (Off). "Ultra" isn't in the public header, so Apex Tune doesn't claim to set it. |
| Power management mode | `0x1057EB71` | 1 = Prefer maximum performance |
| Texture filtering quality | `0x00CE2691` | `0x14` = High performance |
| Threaded optimization (OpenGL) | `0x20C1221E` | 1 = On, 2 = Off |
| Frame rate limiter | `0x10835002` | FPS (0 = off) |
| Vertical sync | `0x00A879CF` | `0x08416747` = Force off, `0x60925292` = app-controlled |
| Shader disk cache size | `0x00AC8497` | MB; default `0x4000` |

Function interface IDs for `nvapi_QueryInterface` were cross-checked against [nvapi-rs](https://github.com/arcnmx/nvapi-rs).

## Custom Apex Tune features (not in the tools above)

- **System Check**: read-only findings with a fix where Windows allows one (refresh rate, VBS state, TPM/Secure Boot for Valorant, RAM speed, GPU MSI mode, low disk space, Game Mode/HAGS).
- **Sticky Keys shortcut off**: pressing Shift five times mid-game no longer pops up a dialog that steals focus.
- **Keep Windows Update from replacing your GPU driver** (`ExcludeWUDriversInQualityUpdate`).
- **Fast Startup off**: a full reboot then actually restarts drivers, which helps after driver updates.
- **Widgets off and Edge background mode off**: removes always-running WebView2 and Edge processes.
- **Timer benchmark**: measures `Sleep(1)` accuracy on your PC instead of assuming 0.5 ms is best.
- **Drift warnings**: if Windows or another tool changes a setting back, the UI shows it.
