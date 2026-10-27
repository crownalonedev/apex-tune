//! System Check: read-only findings about things that matter for gaming but
//! that Windows doesn't surface. Where Windows allows a safe, reversible fix
//! (refresh rate), the finding offers one; the rest explain what to do.

use serde::Serialize;

use crate::sys::{DisplayInfo, Hive, RegData, SysCtx};
use crate::tweaks::atom::Atom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Good,
    Info,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fix {
    /// Switch a display to its highest refresh rate (journaled, revertible).
    RefreshRate { device: String, hz: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub level: Level,
    pub fix: Option<Fix>,
}

fn finding(
    id: impl Into<String>,
    level: Level,
    title: impl Into<String>,
    detail: impl Into<String>,
) -> Finding {
    Finding {
        id: id.into(),
        title: title.into(),
        detail: detail.into(),
        level,
        fix: None,
    }
}

pub fn refresh_findings(displays: &[DisplayInfo]) -> Vec<Finding> {
    displays
        .iter()
        .map(|d| {
            if d.hz + 1 < d.max_hz {
                Finding {
                    fix: Some(Fix::RefreshRate { device: d.device.clone(), hz: d.max_hz }),
                    ..finding(
                        format!("refresh:{}", d.device),
                        Level::Warn,
                        format!("{} runs at {} Hz but supports {} Hz", d.monitor, d.hz, d.max_hz),
                        "Windows often leaves high-refresh monitors at 60 Hz. Switching makes motion smoother and \
                         lowers display latency.",
                    )
                }
            } else {
                finding(
                    format!("refresh:{}", d.device),
                    Level::Good,
                    format!("{} runs at its full {} Hz", d.monitor, d.hz),
                    format!("{}×{} at the highest refresh rate it offers.", d.width, d.height),
                )
            }
        })
        .collect()
}

pub fn refresh_atom(displays: &[DisplayInfo], device: &str, hz: u32) -> Option<Atom> {
    displays
        .iter()
        .find(|d| d.device == device)
        .map(|d| Atom::DisplayRefresh {
            device: d.device.clone(),
            monitor: d.monitor.clone(),
            hz,
        })
}

pub fn secure_boot_finding(enabled: Option<bool>) -> Finding {
    match enabled {
        Some(true) => finding(
            "secure_boot",
            Level::Good,
            "Secure Boot is on",
            "Riot Vanguard needs it on Windows 11, so VALORANT can start.",
        ),
        Some(false) => finding(
            "secure_boot",
            Level::Warn,
            "Secure Boot is off",
            "VALORANT on Windows 11 won't start without it (error VAN 9003). Turn it on in your motherboard's \
             UEFI/BIOS settings; Apex Tune can't change firmware settings.",
        ),
        None => finding(
            "secure_boot",
            Level::Info,
            "Secure Boot state unknown",
            "This PC may use legacy BIOS boot.",
        ),
    }
}

pub fn tpm_finding(present: Option<(bool, String)>) -> Finding {
    match present {
        Some((true, version)) if version.starts_with('2') => finding(
            "tpm",
            Level::Good,
            "TPM 2.0 is ready",
            "Required by Windows 11 and Riot Vanguard.",
        ),
        Some((enabled, version)) => finding(
            "tpm",
            Level::Warn,
            format!(
                "TPM {} {}",
                if version.is_empty() { "?" } else { &version },
                if enabled { "found" } else { "is disabled" }
            ),
            "VALORANT on Windows 11 needs TPM 2.0 enabled (error VAN 9001). Turn on fTPM or PTT in UEFI/BIOS.",
        ),
        None => finding(
            "tpm",
            Level::Warn,
            "No TPM found",
            "VALORANT on Windows 11 needs TPM 2.0 (error VAN 9001).",
        ),
    }
}

pub fn vbs_finding(hvci_running: Option<bool>) -> Finding {
    match hvci_running {
        Some(true) => finding(
            "vbs",
            Level::Info,
            "Memory Integrity (VBS) is on",
            "It protects the Windows kernel and costs 0–6% in most games, more on older CPUs. Apex Tune doesn't \
             change security features; you can review it in Windows Security → Device security → Core isolation.",
        ),
        Some(false) => finding(
            "vbs",
            Level::Info,
            "Memory Integrity (VBS) is off",
            "No virtualization-based overhead.",
        ),
        None => finding(
            "vbs",
            Level::Info,
            "Memory Integrity state unknown",
            "Windows didn't report it.",
        ),
    }
}

/// SMBIOS memory type 26 = DDR4, 34 = DDR5.
pub fn ram_finding(memory_type: u32, configured_mts: u32) -> Option<Finding> {
    let (kind, jedec_ceiling) = match memory_type {
        26 => ("DDR4", 2666),
        34 => ("DDR5", 4800),
        _ => return None,
    };
    Some(if configured_mts <= jedec_ceiling {
        finding(
            "ram",
            Level::Info,
            format!("Memory runs at {configured_mts} MT/s ({kind} default speed)"),
            format!(
                "If your kit is rated faster (it says on the box or label, e.g. {kind}-{}), turn on XMP or EXPO in \
                 UEFI/BIOS. Faster memory noticeably lifts 1% lows in CPU-heavy games.",
                if kind == "DDR4" { 3600 } else { 6000 }
            ),
        )
    } else {
        finding(
            "ram",
            Level::Good,
            format!("Memory runs at {configured_mts} MT/s"),
            "Above the default speed, so an XMP or EXPO profile is on.",
        )
    })
}

pub fn disk_finding(mount: &str, free: u64, total: u64) -> Option<Finding> {
    if total == 0 {
        return None;
    }
    let pct = free as f64 / total as f64 * 100.0;
    Some(if pct < 10.0 {
        finding(
            "disk",
            Level::Warn,
            format!("{mount} is {:.0}% free", pct),
            "Windows and game updates need room to download and unpack. Storage cleanup on the Tweaks page can help.",
        )
    } else {
        finding(
            "disk",
            Level::Good,
            format!("{mount} has {:.0}% free", pct),
            "Plenty of room for updates.",
        )
    })
}

pub fn msi_finding(gpu: &str, msi: Option<bool>) -> Option<Finding> {
    msi.map(|on| {
        if on {
            finding("msi", Level::Good, format!("{gpu} uses message-signaled interrupts"), "The modern, lower-latency interrupt mode.")
        } else {
            finding(
                "msi",
                Level::Info,
                format!("{gpu} uses line-based interrupts"),
                "Recent NVIDIA drivers switch RTX cards to MSI mode themselves. Apex Tune doesn't force it: the driver \
                 resets it on every install and results are mixed.",
            )
        }
    })
}

/// MSISupported for a device, from its PnP instance key.
pub fn msi_enabled(ctx: &SysCtx, pnp_id: &str) -> Option<bool> {
    let key = format!(
        r"SYSTEM\CurrentControlSet\Enum\{pnp_id}\Device Parameters\Interrupt Management\MessageSignaledInterruptProperties"
    );
    match ctx
        .registry
        .get(Hive::LocalMachine, &key, "MSISupported")
        .ok()?
    {
        Some(RegData::Dword(v)) => Some(v == 1),
        _ => Some(false),
    }
}

pub fn secure_boot(ctx: &SysCtx) -> Option<bool> {
    match ctx
        .registry
        .get(
            Hive::LocalMachine,
            r"SYSTEM\CurrentControlSet\Control\SecureBoot\State",
            "UEFISecureBootEnabled",
        )
        .ok()?
    {
        Some(RegData::Dword(v)) => Some(v == 1),
        _ => None,
    }
}

/// Everything System Check reports, worst first.
pub fn run(ctx: &SysCtx, volumes: &[crate::hw::VolumeInfo]) -> Vec<Finding> {
    let mut out = Vec::new();
    if let Ok(displays) = ctx.desktop.displays() {
        out.extend(refresh_findings(&displays));
    }
    out.push(secure_boot_finding(secure_boot(ctx)));
    #[cfg(windows)]
    {
        let probe = win::probe(ctx);
        out.push(tpm_finding(probe.tpm));
        out.push(vbs_finding(probe.hvci));
        if let Some((kind, speed)) = probe.memory {
            out.extend(ram_finding(kind, speed));
        }
        for (gpu, msi) in probe.gpu_msi {
            out.extend(msi_finding(&gpu, msi));
        }
    }
    if let Some(v) = volumes
        .iter()
        .find(|v| v.mount.to_ascii_uppercase().starts_with("C:"))
        .or(volumes.first())
    {
        out.extend(disk_finding(&v.mount, v.free_bytes, v.total_bytes));
    }
    out.sort_by_key(|f| match f.level {
        Level::Warn => 0,
        Level::Info => 1,
        Level::Good => 2,
    });
    out
}

#[cfg(windows)]
mod win {
    use serde::Deserialize;
    use wmi::WMIConnection;

    use crate::sys::SysCtx;

    pub struct Probe {
        pub tpm: Option<(bool, String)>,
        pub hvci: Option<bool>,
        /// (SMBIOS memory type, configured speed in MT/s) of the first module.
        pub memory: Option<(u32, u32)>,
        pub gpu_msi: Vec<(String, Option<bool>)>,
    }

    #[derive(Deserialize)]
    #[serde(rename = "Win32_Tpm")]
    #[allow(non_snake_case)]
    struct Tpm {
        IsEnabled_InitialValue: Option<bool>,
        SpecVersion: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename = "Win32_DeviceGuard")]
    #[allow(non_snake_case)]
    struct DeviceGuard {
        SecurityServicesRunning: Option<Vec<u32>>,
    }

    #[derive(Deserialize)]
    #[serde(rename = "Win32_PhysicalMemory")]
    #[allow(non_snake_case)]
    struct Memory {
        SMBIOSMemoryType: Option<u32>,
        ConfiguredClockSpeed: Option<u32>,
    }

    #[derive(Deserialize)]
    #[serde(rename = "Win32_VideoController")]
    #[allow(non_snake_case)]
    struct Video {
        Name: Option<String>,
        PNPDeviceID: Option<String>,
    }

    pub fn probe(ctx: &SysCtx) -> Probe {
        let tpm = WMIConnection::with_namespace_path(r"ROOT\CIMV2\Security\MicrosoftTpm")
            .and_then(|c| c.query::<Tpm>())
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .map(|t| {
                let version = t
                    .SpecVersion
                    .unwrap_or_default()
                    .split(',')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                (t.IsEnabled_InitialValue.unwrap_or(false), version)
            });
        // SecurityServicesRunning: 2 = Hypervisor-protected Code Integrity (Memory Integrity).
        let hvci = WMIConnection::with_namespace_path(r"ROOT\Microsoft\Windows\DeviceGuard")
            .and_then(|c| c.query::<DeviceGuard>())
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .map(|d| d.SecurityServicesRunning.unwrap_or_default().contains(&2));
        let cimv2 = WMIConnection::new().ok();
        let memory = cimv2
            .as_ref()
            .and_then(|c| c.query::<Memory>().ok())
            .and_then(|rows| rows.into_iter().next())
            .and_then(|m| Some((m.SMBIOSMemoryType?, m.ConfiguredClockSpeed?)));
        let gpu_msi = cimv2
            .as_ref()
            .and_then(|c| c.query::<Video>().ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| {
                let pnp = v.PNPDeviceID?;
                pnp.to_ascii_uppercase().starts_with("PCI\\").then(|| {
                    (
                        v.Name.unwrap_or_else(|| "GPU".into()),
                        super::msi_enabled(ctx, &pnp),
                    )
                })
            })
            .collect();
        Probe {
            tpm,
            hvci,
            memory,
            gpu_msi,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::Engine;

    fn display(hz: u32, max_hz: u32) -> DisplayInfo {
        DisplayInfo {
            device: r"\\.\DISPLAY1".into(),
            monitor: "XG27".into(),
            width: 2560,
            height: 1440,
            hz,
            max_hz,
        }
    }

    #[test]
    fn flags_monitors_below_their_max_and_fix_is_reversible() {
        let f = refresh_findings(&[display(60, 165)]);
        assert_eq!(f[0].level, Level::Warn);
        assert_eq!(
            f[0].fix,
            Some(Fix::RefreshRate {
                device: r"\\.\DISPLAY1".into(),
                hz: 165
            })
        );
        assert_eq!(refresh_findings(&[display(144, 144)])[0].level, Level::Good);
        // 59.94 vs 60 style rounding isn't flagged.
        assert_eq!(refresh_findings(&[display(59, 60)])[0].level, Level::Good);

        let sys = FakeSystem::new();
        sys.desktop.displays.lock().unwrap().push(display(60, 165));
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        let displays = sys.ctx.desktop.displays().unwrap();
        let atom = refresh_atom(&displays, r"\\.\DISPLAY1", 165).unwrap();
        engine
            .apply_action("display.refresh", "165 Hz", vec![atom])
            .unwrap();
        assert_eq!(sys.ctx.desktop.displays().unwrap()[0].hz, 165);
        engine.revert_everything();
        assert_eq!(sys.ctx.desktop.displays().unwrap()[0].hz, 60);
    }

    #[test]
    fn ram_and_disk_findings() {
        assert_eq!(ram_finding(34, 4800).unwrap().level, Level::Info);
        assert_eq!(ram_finding(34, 6000).unwrap().level, Level::Good);
        assert_eq!(ram_finding(26, 3200).unwrap().level, Level::Good);
        assert!(ram_finding(0, 1600).is_none());
        assert_eq!(disk_finding("C:\\", 5, 100).unwrap().level, Level::Warn);
        assert_eq!(disk_finding("C:\\", 50, 100).unwrap().level, Level::Good);
    }

    #[test]
    fn valorant_requirements() {
        assert_eq!(secure_boot_finding(Some(false)).level, Level::Warn);
        assert_eq!(tpm_finding(Some((true, "2.0".into()))).level, Level::Good);
        assert_eq!(tpm_finding(Some((true, "1.2".into()))).level, Level::Warn);
        assert_eq!(vbs_finding(Some(true)).level, Level::Info);
    }

    #[test]
    fn findings_are_sorted_worst_first() {
        let sys = FakeSystem::new();
        sys.desktop.displays.lock().unwrap().push(display(60, 144));
        let f = run(&sys.ctx, &[]);
        assert_eq!(f[0].level, Level::Warn);
    }
}
