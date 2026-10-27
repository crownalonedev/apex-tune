use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::safety::{EngineState, blocking};
use crate::error::{AppError, AppResult};
use crate::settings::{self, Settings};
use crate::tweaks::TweakError;

/// The current settings, shared with the window-close handler.
#[derive(Default)]
pub struct SettingsState(pub Mutex<Settings>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    pub start_with_windows: bool,
    pub data_dir: String,
    pub version: &'static str,
}

fn autostart_on(e: &crate::tweaks::Engine) -> AppResult<bool> {
    let exe = std::env::current_exe()?;
    let mut on = true;
    for atom in settings::autostart_atoms(&exe) {
        on &= atom.is_applied(e.ctx()).map_err(TweakError::from)?;
    }
    Ok(on)
}

#[tauri::command]
pub async fn settings_get(
    state: State<'_, EngineState>,
    current: State<'_, SettingsState>,
) -> AppResult<SettingsView> {
    let settings = current.0.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let start_with_windows = blocking(state, |e| autostart_on(e)).await.unwrap_or(false);
    Ok(SettingsView {
        settings,
        start_with_windows,
        data_dir: crate::paths::data_root().display().to_string(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tauri::command]
pub fn settings_set(current: State<'_, SettingsState>, settings: Settings) -> AppResult<()> {
    settings::save(&settings)?;
    *current.0.lock().unwrap_or_else(|p| p.into_inner()) = settings;
    Ok(())
}

#[tauri::command]
pub async fn settings_set_autostart(state: State<'_, EngineState>, enabled: bool) -> AppResult<()> {
    blocking(state, move |e| {
        if enabled {
            let exe = std::env::current_exe()?;
            e.apply_action(
                settings::AUTOSTART_ACTION,
                "Start Apex Tune with Windows",
                settings::autostart_atoms(&exe),
            )?;
        } else {
            e.revert_action(settings::AUTOSTART_ACTION)?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn open_logs_folder(app: AppHandle) -> AppResult<()> {
    app.opener()
        .open_path(crate::paths::logs_dir().display().to_string(), None::<&str>)
        .map_err(|e| AppError::Other(format!("Couldn't open the logs folder: {e}")))
}
