//! Hardware and OS detection for the Dashboard.
//!
//! Parsing and lookups that don't need WMI read through [`SysCtx`] so they're
//! unit-tested with the fake registry. WMI queries live in [`win`].

pub mod gpu;
pub mod stats;
#[cfg(windows)]
mod win;
pub mod windows_version;

use serde::Serialize;
use sysinfo::{Disks, System};

use crate::sys::SysCtx;
pub use gpu::{GpuInfo, GpuVendor};
pub use windows_version::WindowsVersion;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub name: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
    /// Base clock reported by Windows, in MHz.
    pub base_mhz: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DriveKind {
    NvmeSsd,
    Ssd,
    Hdd,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub name: String,
    pub kind: DriveKind,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    pub mount: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSummary {
    pub cpu: CpuInfo,
    pub memory_bytes: u64,
    pub gpus: Vec<GpuInfo>,
    pub drives: Vec<DriveInfo>,
    pub volumes: Vec<VolumeInfo>,
    pub windows: Option<WindowsVersion>,
    /// A battery means a laptop: the power plan stage offers its laptop variant.
    pub has_battery: bool,
    /// Parts that couldn't be read, so the UI can say so instead of guessing.
    pub warnings: Vec<String>,
}

pub fn detect(ctx: &SysCtx) -> HardwareSummary {
    let mut warnings = Vec::new();

    let mut system = System::new();
    system.refresh_cpu_all();
    system.refresh_memory();
    let cpu = CpuInfo {
        name: system
            .cpus()
            .first()
            .map_or_else(|| "Unknown CPU".into(), |c| c.brand().trim().to_owned()),
        physical_cores: System::physical_core_count(),
        logical_cores: system.cpus().len(),
        base_mhz: system.cpus().first().map_or(0, |c| c.frequency()),
    };

    let volumes = Disks::new_with_refreshed_list()
        .list()
        .iter()
        .filter(|d| !d.is_removable())
        .map(|d| VolumeInfo {
            mount: d.mount_point().display().to_string(),
            total_bytes: d.total_space(),
            free_bytes: d.available_space(),
        })
        .collect();

    let windows = match windows_version::read(ctx) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("Windows version: {e}"));
            None
        }
    };

    #[cfg(windows)]
    let (gpus, drives, has_battery) = {
        let probe = win::probe(ctx);
        warnings.extend(probe.warnings);
        (probe.gpus, probe.drives, probe.has_battery)
    };
    #[cfg(not(windows))]
    let (gpus, drives, has_battery) = (Vec::new(), Vec::new(), false);

    HardwareSummary {
        cpu,
        memory_bytes: system.total_memory(),
        gpus,
        drives,
        volumes,
        windows,
        has_battery,
        warnings,
    }
}

/// MSFT_PhysicalDisk `BusType` and `MediaType` to a drive kind.
pub fn drive_kind(bus_type: u16, media_type: u16) -> DriveKind {
    const BUS_NVME: u16 = 17;
    const MEDIA_HDD: u16 = 3;
    const MEDIA_SSD: u16 = 4;
    match (bus_type, media_type) {
        (BUS_NVME, _) => DriveKind::NvmeSsd,
        (_, MEDIA_SSD) => DriveKind::Ssd,
        (_, MEDIA_HDD) => DriveKind::Hdd,
        _ => DriveKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_kinds() {
        assert_eq!(drive_kind(17, 0), DriveKind::NvmeSsd);
        assert_eq!(drive_kind(11, 4), DriveKind::Ssd);
        assert_eq!(drive_kind(11, 3), DriveKind::Hdd);
        assert_eq!(drive_kind(7, 0), DriveKind::Unknown);
    }

    #[test]
    fn detect_runs_without_windows() {
        let sys = crate::sys::fake::FakeSystem::new();
        let summary = detect(&sys.ctx);
        assert!(summary.cpu.logical_cores > 0);
        assert!(summary.memory_bytes > 0);
    }
}
