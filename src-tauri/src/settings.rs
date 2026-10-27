//! App preferences, stored as JSON next to the backups.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Follows Windows' light/dark setting.
    System,
    Graphite,
    Ember,
    Moss,
    Violet,
    Oled,
    Daylight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Motion {
    /// Follows Windows' "Animation effects" setting.
    System,
    Full,
    Reduced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    pub motion: Motion,
    /// Closing the window keeps Apex Tune in the tray.
    pub minimize_to_tray: bool,
    pub check_for_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Graphite,
            motion: Motion::System,
            minimize_to_tray: false,
            check_for_updates: true,
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::paths::data_root().join("settings.json")
}

pub fn load() -> Settings {
    std::fs::read(path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    std::fs::write(path(), serde_json::to_vec_pretty(settings)?)
}

pub const AUTOSTART_ACTION: &str = "app.autostart";
pub const AUTOSTART_TASK: &str = r"\ApexTune\App";
/// Passed by the sign-in task so Apex Tune starts in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

pub fn autostart_atoms(exe: &std::path::Path) -> Vec<crate::tweaks::atom::Atom> {
    vec![crate::tweaks::atom::Atom::LogonTask {
        path: AUTOSTART_TASK.into(),
        command: exe.display().to_string(),
        arguments: MINIMIZED_ARG.into(),
        description: "Starts Apex Tune in the tray when you sign in.".into(),
        // Apex Tune needs admin rights, and Windows won't elevate Run-key entries.
        highest: true,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"theme":"ember"}"#).unwrap();
        assert_eq!(s.theme, Theme::Ember);
        assert!(s.check_for_updates);
    }

    #[test]
    fn autostart_task_passes_the_guard_only_for_apex_tune_itself() {
        let ok = autostart_atoms(std::path::Path::new(
            r"C:\Program Files\Apex Tune\apex-tune.exe",
        ));
        crate::guard::check_atom(&ok[0]).unwrap();
        let other = autostart_atoms(std::path::Path::new(r"C:\Temp\evil.exe"));
        assert!(crate::guard::check_atom(&other[0]).is_err());
    }
}
