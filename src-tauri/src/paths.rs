//! Locations of everything Apex Tune writes to disk.
//!
//! On Windows the root is `%ProgramData%\ApexTune`. Elsewhere (development and
//! unit tests on other OSes) it is `APEX_TUNE_DATA_DIR` or a folder in the temp dir.

use std::path::PathBuf;

const ROOT_ENV_OVERRIDE: &str = "APEX_TUNE_DATA_DIR";

pub fn data_root() -> PathBuf {
    if let Some(dir) = std::env::var_os(ROOT_ENV_OVERRIDE) {
        return PathBuf::from(dir);
    }
    platform_root()
}

#[cfg(windows)]
fn platform_root() -> PathBuf {
    let program_data = std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into());
    PathBuf::from(program_data).join("ApexTune")
}

#[cfg(not(windows))]
fn platform_root() -> PathBuf {
    std::env::temp_dir().join("ApexTune-dev")
}

pub fn backups_dir() -> PathBuf {
    data_root().join("backups")
}

pub fn logs_dir() -> PathBuf {
    data_root().join("logs")
}

pub fn profiles_dir() -> PathBuf {
    data_root().join("profiles")
}

/// Creates the data folders and, on Windows, restricts them so only
/// Administrators and SYSTEM can write. Backups decide what an elevated
/// process writes to the registry, so a standard user must not be able to
/// plant or edit them.
pub fn ensure_dirs() -> std::io::Result<()> {
    let root = data_root();
    std::fs::create_dir_all(&root)?;
    crate::platform::restrict_to_admins(&root)?;
    for dir in [backups_dir(), logs_dir(), profiles_dir()] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
