use serde::Serialize;

use crate::sys::{Hive, RegData, SysCtx};

/// Display adapters' driver keys (the "Display" device class).
const DISPLAY_CLASS: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    /// Windows driver version, e.g. "32.0.15.6094".
    pub driver_version: String,
    /// NVIDIA's own numbering, e.g. "560.94".
    pub nvidia_driver: Option<String>,
    pub vram_bytes: Option<u64>,
}

pub fn vendor_from(adapter_compatibility: &str, name: &str) -> GpuVendor {
    let s = format!("{adapter_compatibility} {name}").to_ascii_lowercase();
    if s.contains("nvidia") {
        GpuVendor::Nvidia
    } else if s.contains("advanced micro devices") || s.contains("amd") || s.contains("radeon") {
        GpuVendor::Amd
    } else if s.contains("intel") {
        GpuVendor::Intel
    } else {
        GpuVendor::Other
    }
}

/// NVIDIA's version is the last five digits of the Windows driver version:
/// 32.0.15.6094 → 15 6094 → 56094 → 560.94.
pub fn nvidia_driver_version(windows_version: &str) -> Option<String> {
    let parts: Vec<&str> = windows_version.split('.').collect();
    let [.., third, fourth] = parts.as_slice() else {
        return None;
    };
    if !third
        .chars()
        .chain(fourth.chars())
        .all(|c| c.is_ascii_digit())
        || fourth.len() > 4
    {
        return None;
    }
    let digits = format!("{third}{fourth:0>4}");
    let tail = &digits[digits.len().checked_sub(5)?..];
    Some(format!("{}.{}", &tail[..3], &tail[3..]))
}

/// Dedicated video memory from the adapter's driver key. WMI's AdapterRAM is
/// a 32-bit field that caps at 4 GB, so it isn't used.
pub fn vram_from_registry(ctx: &SysCtx, gpu_name: &str) -> Option<u64> {
    let subkeys = ctx
        .registry
        .subkeys(Hive::LocalMachine, DISPLAY_CLASS)
        .ok()?;
    subkeys
        .iter()
        .filter(|k| k.len() == 4 && k.chars().all(|c| c.is_ascii_digit()))
        .find_map(|k| {
            let path = format!(r"{DISPLAY_CLASS}\{k}");
            let desc = ctx
                .registry
                .get(Hive::LocalMachine, &path, "DriverDesc")
                .ok()??;
            if !matches!(&desc, RegData::Sz(d) if d.eq_ignore_ascii_case(gpu_name)) {
                return None;
            }
            [
                "HardwareInformation.qwMemorySize",
                "HardwareInformation.MemorySize",
            ]
            .iter()
            .find_map(
                |name| match ctx.registry.get(Hive::LocalMachine, &path, name).ok()?? {
                    RegData::Qword(v) => Some(v),
                    RegData::Dword(v) => Some(u64::from(v)),
                    RegData::Binary(b) if b.len() == 8 => {
                        Some(u64::from_le_bytes(b.try_into().ok()?))
                    }
                    RegData::Binary(b) if b.len() == 4 => {
                        Some(u64::from(u32::from_le_bytes(b.try_into().ok()?)))
                    }
                    _ => None,
                },
            )
            .filter(|v| *v > 0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::fake::FakeSystem;

    #[test]
    fn nvidia_versions() {
        assert_eq!(
            nvidia_driver_version("32.0.15.6094").as_deref(),
            Some("560.94")
        );
        assert_eq!(
            nvidia_driver_version("31.0.15.5222").as_deref(),
            Some("552.22")
        );
        assert_eq!(
            nvidia_driver_version("32.0.16.0512").as_deref(),
            Some("605.12")
        );
        assert_eq!(nvidia_driver_version("garbage"), None);
    }

    #[test]
    fn vendors() {
        assert_eq!(
            vendor_from("NVIDIA", "NVIDIA GeForce RTX 4070"),
            GpuVendor::Nvidia
        );
        assert_eq!(
            vendor_from("Advanced Micro Devices, Inc.", "AMD Radeon RX 7800 XT"),
            GpuVendor::Amd
        );
        assert_eq!(
            vendor_from("Intel Corporation", "Intel(R) UHD Graphics 770"),
            GpuVendor::Intel
        );
        assert_eq!(
            vendor_from("Microsoft", "Microsoft Basic Display Adapter"),
            GpuVendor::Other
        );
    }

    #[test]
    fn vram_reads_the_matching_adapter() {
        let sys = FakeSystem::new();
        let a = format!(r"{DISPLAY_CLASS}\0000");
        let b = format!(r"{DISPLAY_CLASS}\0001");
        sys.registry.seed(
            Hive::LocalMachine,
            &a,
            "DriverDesc",
            RegData::Sz("Intel(R) UHD Graphics 770".into()),
        );
        sys.registry.seed(
            Hive::LocalMachine,
            &a,
            "HardwareInformation.MemorySize",
            RegData::Binary(vec![0, 0, 0, 0x20]),
        );
        sys.registry.seed(
            Hive::LocalMachine,
            &b,
            "DriverDesc",
            RegData::Sz("NVIDIA GeForce RTX 4070".into()),
        );
        sys.registry.seed(
            Hive::LocalMachine,
            &b,
            "HardwareInformation.qwMemorySize",
            RegData::Qword(12 << 30),
        );
        sys.registry
            .seed_key(Hive::LocalMachine, &format!(r"{DISPLAY_CLASS}\Properties"));

        assert_eq!(
            vram_from_registry(&sys.ctx, "NVIDIA GeForce RTX 4070"),
            Some(12 << 30)
        );
        assert_eq!(
            vram_from_registry(&sys.ctx, "Intel(R) UHD Graphics 770"),
            Some(0x2000_0000)
        );
        assert_eq!(vram_from_registry(&sys.ctx, "Missing GPU"), None);
    }
}
