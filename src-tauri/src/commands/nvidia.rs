use serde::Serialize;
use tauri::State;

use super::safety::{EngineState, blocking};
use crate::error::{AppError, AppResult};
use crate::nvidia::{self, GameProfile};
use crate::sys::NvInfo;
use crate::tweaks::{Engine, TweakError};

fn sys<T>(r: crate::sys::SysResult<T>) -> AppResult<T> {
    r.map_err(|e| AppError::from(TweakError::from(e)))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NvidiaStatus {
    pub info: Option<NvInfo>,
    pub games: Vec<GameProfile>,
    pub has_backup: bool,
}

fn status(e: &Engine) -> AppResult<NvidiaStatus> {
    let info = sys(e.ctx().nvidia.info())?;
    let games = if info.is_some() {
        let list = sys(crate::games::list(e.ctx()))?;
        sys(nvidia::game_profiles(e.ctx(), &list))?
    } else {
        Vec::new()
    };
    Ok(NvidiaStatus {
        info,
        games,
        has_backup: nvidia::latest_backup().is_some(),
    })
}

fn backup(e: &mut Engine) -> AppResult<()> {
    let session = e.session_id()?;
    sys(nvidia::ensure_backup(e.ctx(), session)).map(|_| ())
}

#[tauri::command]
pub async fn nvidia_status(state: State<'_, EngineState>) -> AppResult<NvidiaStatus> {
    blocking(state, |e| status(e)).await
}

#[tauri::command]
pub async fn nvidia_set_game(
    state: State<'_, EngineState>,
    exe: String,
    on: bool,
) -> AppResult<NvidiaStatus> {
    blocking(state, move |e| {
        let id = nvidia::game_action(&exe);
        if on {
            backup(e)?;
            e.apply_action(
                &id,
                &format!("NVIDIA low-latency profile for {exe}"),
                nvidia::game_atoms(&exe),
            )?;
        } else {
            e.revert_action(&id)?;
        }
        status(e)
    })
    .await
}

/// Sets (or with `fps = 0`, removes) a driver frame cap for one game.
#[tauri::command]
pub async fn nvidia_set_fps(
    state: State<'_, EngineState>,
    exe: String,
    fps: u32,
) -> AppResult<NvidiaStatus> {
    blocking(state, move |e| {
        let id = nvidia::fps_action(&exe);
        e.revert_action(&id)?;
        if fps > 0 {
            backup(e)?;
            e.apply_action(
                &id,
                &format!("NVIDIA frame cap {fps} FPS for {exe}"),
                nvidia::fps_atoms(&exe, fps),
            )?;
        }
        status(e)
    })
    .await
}

/// Puts the whole driver settings database back to the newest full backup.
#[tauri::command]
pub async fn nvidia_restore_backup(state: State<'_, EngineState>) -> AppResult<()> {
    blocking(state, |e| {
        let path = nvidia::latest_backup().ok_or_else(|| AppError::Other("No NVIDIA backup exists yet.".into()))?;
        sys(e.ctx().nvidia.import(&path))?;
        tracing::info!(target: crate::log::ACTIVITY_TARGET, tweak = "nvidia", "Restored all NVIDIA settings from {}", path.display());
        Ok(())
    })
    .await
}
