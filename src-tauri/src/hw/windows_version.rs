use serde::Serialize;

use crate::sys::{Hive, RegData, SysCtx, SysResult};

const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const FIRST_WINDOWS_11_BUILD: u32 = 22000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsVersion {
    /// e.g. "Windows 11 Pro".
    pub product_name: String,
    /// e.g. "24H2".
    pub display_version: Option<String>,
    pub build: u32,
    /// Update build revision, the part after the dot in 26100.2605.
    pub revision: Option<u32>,
    /// e.g. "Professional", "Core" (Home).
    pub edition: Option<String>,
}

impl WindowsVersion {
    pub fn is_windows_11(&self) -> bool {
        self.build >= FIRST_WINDOWS_11_BUILD
    }

    /// Home editions ignore most Group Policy keys (e.g. deferring feature updates).
    pub fn is_home(&self) -> bool {
        self.edition
            .as_deref()
            .is_some_and(|e| e.starts_with("Core"))
    }
}

fn string(ctx: &SysCtx, name: &str) -> SysResult<Option<String>> {
    Ok(
        match ctx
            .registry
            .get(Hive::LocalMachine, CURRENT_VERSION, name)?
        {
            Some(RegData::Sz(s) | RegData::ExpandSz(s)) if !s.is_empty() => Some(s),
            _ => None,
        },
    )
}

/// Reads the version from the registry. `Ok(None)` if the key isn't there
/// (not Windows).
pub fn read(ctx: &SysCtx) -> SysResult<Option<WindowsVersion>> {
    let Some(build) = string(ctx, "CurrentBuildNumber")?.and_then(|b| b.parse::<u32>().ok()) else {
        return Ok(None);
    };
    let revision = match ctx
        .registry
        .get(Hive::LocalMachine, CURRENT_VERSION, "UBR")?
    {
        Some(RegData::Dword(v)) => Some(v),
        _ => None,
    };
    let mut product_name = string(ctx, "ProductName")?.unwrap_or_else(|| "Windows".into());
    // Windows 11 still reports "Windows 10" in ProductName for compatibility.
    if build >= FIRST_WINDOWS_11_BUILD && product_name.starts_with("Windows 10") {
        product_name = product_name.replacen("Windows 10", "Windows 11", 1);
    }
    Ok(Some(WindowsVersion {
        product_name,
        display_version: string(ctx, "DisplayVersion")?,
        build,
        revision,
        edition: string(ctx, "EditionID")?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::fake::FakeSystem;

    fn seed(sys: &FakeSystem, name: &str, data: RegData) {
        sys.registry
            .seed(Hive::LocalMachine, CURRENT_VERSION, name, data);
    }

    #[test]
    fn corrects_the_windows_10_product_name_on_11() {
        let sys = FakeSystem::new();
        seed(&sys, "ProductName", RegData::Sz("Windows 10 Pro".into()));
        seed(&sys, "CurrentBuildNumber", RegData::Sz("26100".into()));
        seed(&sys, "UBR", RegData::Dword(2605));
        seed(&sys, "DisplayVersion", RegData::Sz("24H2".into()));
        seed(&sys, "EditionID", RegData::Sz("Professional".into()));

        let v = read(&sys.ctx).unwrap().unwrap();
        assert_eq!(v.product_name, "Windows 11 Pro");
        assert_eq!((v.build, v.revision), (26100, Some(2605)));
        assert!(v.is_windows_11());
        assert!(!v.is_home());
    }

    #[test]
    fn keeps_windows_10_and_detects_home() {
        let sys = FakeSystem::new();
        seed(&sys, "ProductName", RegData::Sz("Windows 10 Home".into()));
        seed(&sys, "CurrentBuildNumber", RegData::Sz("19045".into()));
        seed(&sys, "EditionID", RegData::Sz("Core".into()));
        let v = read(&sys.ctx).unwrap().unwrap();
        assert_eq!(v.product_name, "Windows 10 Home");
        assert!(v.is_home());
    }

    #[test]
    fn none_when_not_windows() {
        assert_eq!(read(&FakeSystem::new().ctx).unwrap(), None);
    }
}
