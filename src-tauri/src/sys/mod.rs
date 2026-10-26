//! The only layer that touches the operating system.
//!
//! Each OS facility is a trait with a real Windows implementation in [`win`]
//! and an in-memory one in [`fake`] for unit tests, so tweak, journal and
//! rollback logic is tested on any OS without changing anything real.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[cfg(any(test, feature = "fake-sys"))]
pub mod fake;
#[cfg(windows)]
pub mod win;

// ---- Errors -----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SysErrorKind {
    NotFound,
    AccessDenied,
    Unsupported,
    Other,
}

/// A failed OS call: what was attempted, on what, and why it failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{op} {target}: {message}")]
pub struct SysError {
    pub kind: SysErrorKind,
    pub op: &'static str,
    pub target: String,
    pub message: String,
}

impl SysError {
    pub fn new(
        kind: SysErrorKind,
        op: &'static str,
        target: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            op,
            target: target.into(),
            message: message.into(),
        }
    }
}

pub type SysResult<T> = Result<T, SysError>;

// ---- Registry ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Hive {
    /// HKEY_LOCAL_MACHINE
    #[serde(rename = "HKLM")]
    LocalMachine,
    /// The interactive user's hive. When Apex Tune is elevated with a
    /// different admin account, this is still the signed-in user's hive.
    #[serde(rename = "HKCU")]
    CurrentUser,
}

impl fmt::Display for Hive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Hive::LocalMachine => "HKLM",
            Hive::CurrentUser => "HKCU",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum RegData {
    Dword(u32),
    Qword(u64),
    Sz(String),
    ExpandSz(String),
    MultiSz(Vec<String>),
    Binary(Vec<u8>),
    /// Any other registry type, kept byte-for-byte so it can be restored exactly.
    Other {
        ty: u32,
        bytes: Vec<u8>,
    },
}

impl fmt::Display for RegData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegData::Dword(v) => write!(f, "{v} (0x{v:X})"),
            RegData::Qword(v) => write!(f, "{v} (0x{v:X})"),
            RegData::Sz(s) | RegData::ExpandSz(s) => write!(f, "\"{s}\""),
            RegData::MultiSz(items) => write!(f, "[{}]", items.join(", ")),
            RegData::Other { ty, bytes } => write!(f, "type {ty}, {} bytes", bytes.len()),
            RegData::Binary(bytes) => {
                let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
                write!(f, "{}", hex.join(" "))
            }
        }
    }
}

pub trait Registry: Send + Sync {
    /// `Ok(None)` when the value or any key on the path doesn't exist.
    fn get(&self, hive: Hive, path: &str, name: &str) -> SysResult<Option<RegData>>;
    fn key_exists(&self, hive: Hive, path: &str) -> SysResult<bool>;
    /// Creates missing keys on the way.
    fn set(&self, hive: Hive, path: &str, name: &str, data: &RegData) -> SysResult<()>;
    /// Succeeds if the value is already absent.
    fn delete_value(&self, hive: Hive, path: &str, name: &str) -> SysResult<()>;
    /// Deletes the key only if it has no values and no subkeys. Returns
    /// whether it was deleted. Succeeds (false) if the key is absent.
    fn delete_key_if_empty(&self, hive: Hive, path: &str) -> SysResult<bool>;
    fn subkeys(&self, hive: Hive, path: &str) -> SysResult<Vec<String>>;
}

// ---- Services ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartType {
    Boot,
    System,
    Automatic,
    AutomaticDelayed,
    Manual,
    Disabled,
}

impl fmt::Display for StartType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StartType::Boot => "Boot",
            StartType::System => "System",
            StartType::Automatic => "Automatic",
            StartType::AutomaticDelayed => "Automatic (delayed start)",
            StartType::Manual => "Manual",
            StartType::Disabled => "Disabled",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceState {
    pub start: StartType,
    pub running: bool,
}

pub trait Services: Send + Sync {
    /// `Ok(None)` when the service isn't installed.
    fn query(&self, name: &str) -> SysResult<Option<ServiceState>>;
    fn set_start(&self, name: &str, start: StartType) -> SysResult<()>;
    /// Stops the service and waits for it to stop. Succeeds if already stopped.
    fn stop(&self, name: &str) -> SysResult<()>;
    /// Starts the service. Succeeds if already running.
    fn start(&self, name: &str) -> SysResult<()>;
}

// ---- Restore points ---------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePoint {
    pub sequence: u32,
    pub description: String,
    /// As reported by Windows (WMI datetime), shown verbatim if unparsable.
    pub created: String,
}

pub trait RestorePoints: Send + Sync {
    /// Whether System Protection is on for the system drive. `None` if unknown.
    fn protection_enabled(&self) -> SysResult<Option<bool>>;
    fn enable_protection(&self) -> SysResult<()>;
    /// Creates a restore point and returns it.
    fn create(&self, description: &str) -> SysResult<RestorePoint>;
    fn list(&self) -> SysResult<Vec<RestorePoint>>;
}

// ---- Power ------------------------------------------------------------------

/// GUIDs are lower-case hyphenated strings, e.g. "381b4222-f694-41f0-9685-ff5bb260df2e".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerScheme {
    pub guid: String,
    pub name: String,
}

/// Which power source a setting value applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerSource {
    /// Plugged in.
    Ac,
    /// On battery.
    Dc,
}

pub trait Power: Send + Sync {
    fn schemes(&self) -> SysResult<Vec<PowerScheme>>;
    fn active(&self) -> SysResult<String>;
    fn set_active(&self, guid: &str) -> SysResult<()>;
    fn exists(&self, guid: &str) -> SysResult<bool>;
    /// Copies `source` (an installed plan or a built-in template) to a new plan with the given GUID.
    fn duplicate(&self, source: &str, dest: &str) -> SysResult<()>;
    /// Fails if the plan is active.
    fn delete(&self, guid: &str) -> SysResult<()>;
    fn rename(&self, guid: &str, name: &str, description: &str) -> SysResult<()>;
    /// `Ok(None)` when the plan or setting doesn't exist.
    fn read(
        &self,
        scheme: &str,
        subgroup: &str,
        setting: &str,
        source: PowerSource,
    ) -> SysResult<Option<u32>>;
    /// Re-applies the plan if it's active, so the change takes effect now.
    fn write(
        &self,
        scheme: &str,
        subgroup: &str,
        setting: &str,
        source: PowerSource,
        value: u32,
    ) -> SysResult<()>;
    fn has_battery(&self) -> SysResult<bool>;
}

// ---- Context ----------------------------------------------------------------

/// Everything a tweak may use to read or change the system.
#[derive(Clone)]
pub struct SysCtx {
    pub registry: Arc<dyn Registry>,
    pub services: Arc<dyn Services>,
    pub restore_points: Arc<dyn RestorePoints>,
    pub power: Arc<dyn Power>,
}

impl SysCtx {
    /// The real backends for this OS. On non-Windows builds every call
    /// returns `Unsupported`, so the UI still runs in development.
    pub fn native() -> Self {
        #[cfg(windows)]
        {
            Self {
                registry: Arc::new(win::registry::WinRegistry::new()),
                services: Arc::new(win::services::WinServices),
                restore_points: Arc::new(win::restore::WinRestorePoints),
                power: Arc::new(win::power::WinPower),
            }
        }
        #[cfg(not(windows))]
        {
            let unsupported = Arc::new(unsupported::Unsupported);
            Self {
                registry: unsupported.clone(),
                services: unsupported.clone(),
                restore_points: unsupported.clone(),
                power: unsupported,
            }
        }
    }
}

#[cfg(not(windows))]
mod unsupported {
    use super::*;

    pub struct Unsupported;

    fn err<T>(op: &'static str) -> SysResult<T> {
        Err(SysError::new(
            SysErrorKind::Unsupported,
            op,
            "",
            "only available on Windows",
        ))
    }

    impl Registry for Unsupported {
        fn get(&self, _: Hive, _: &str, _: &str) -> SysResult<Option<RegData>> {
            err("read registry")
        }
        fn key_exists(&self, _: Hive, _: &str) -> SysResult<bool> {
            err("read registry")
        }
        fn set(&self, _: Hive, _: &str, _: &str, _: &RegData) -> SysResult<()> {
            err("write registry")
        }
        fn delete_value(&self, _: Hive, _: &str, _: &str) -> SysResult<()> {
            err("write registry")
        }
        fn delete_key_if_empty(&self, _: Hive, _: &str) -> SysResult<bool> {
            err("write registry")
        }
        fn subkeys(&self, _: Hive, _: &str) -> SysResult<Vec<String>> {
            err("read registry")
        }
    }

    impl Services for Unsupported {
        fn query(&self, _: &str) -> SysResult<Option<ServiceState>> {
            err("query service")
        }
        fn set_start(&self, _: &str, _: StartType) -> SysResult<()> {
            err("configure service")
        }
        fn stop(&self, _: &str) -> SysResult<()> {
            err("stop service")
        }
        fn start(&self, _: &str) -> SysResult<()> {
            err("start service")
        }
    }

    impl RestorePoints for Unsupported {
        fn protection_enabled(&self) -> SysResult<Option<bool>> {
            Ok(None)
        }
        fn enable_protection(&self) -> SysResult<()> {
            err("enable System Protection")
        }
        fn create(&self, _: &str) -> SysResult<RestorePoint> {
            err("create restore point")
        }
        fn list(&self) -> SysResult<Vec<RestorePoint>> {
            Ok(Vec::new())
        }
    }

    impl Power for Unsupported {
        fn schemes(&self) -> SysResult<Vec<PowerScheme>> {
            err("list power plans")
        }
        fn active(&self) -> SysResult<String> {
            err("read active power plan")
        }
        fn set_active(&self, _: &str) -> SysResult<()> {
            err("switch power plan")
        }
        fn exists(&self, _: &str) -> SysResult<bool> {
            err("read power plan")
        }
        fn duplicate(&self, _: &str, _: &str) -> SysResult<()> {
            err("create power plan")
        }
        fn delete(&self, _: &str) -> SysResult<()> {
            err("delete power plan")
        }
        fn rename(&self, _: &str, _: &str, _: &str) -> SysResult<()> {
            err("rename power plan")
        }
        fn read(&self, _: &str, _: &str, _: &str, _: PowerSource) -> SysResult<Option<u32>> {
            err("read power setting")
        }
        fn write(&self, _: &str, _: &str, _: &str, _: PowerSource, _: u32) -> SysResult<()> {
            err("write power setting")
        }
        fn has_battery(&self) -> SysResult<bool> {
            Ok(false)
        }
    }
}
