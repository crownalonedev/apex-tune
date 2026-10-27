use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use super::safety::{EngineState, blocking};
use super::tweaks::PRESET_PROGRESS_EVENT;
use crate::backup::journal::EntryRef;
use crate::backup::view::{self, SessionView};
use crate::error::{AppError, AppResult};
use crate::profiles::{self, Profile};
use crate::tweaks::TweakState;
use crate::tweaks::preset::{self, BatchReport, BatchResult, BatchStatus};

#[tauri::command]
pub async fn backups_sessions(state: State<'_, EngineState>) -> AppResult<Vec<SessionView>> {
    blocking(state, |e| Ok(view::sessions(e.journal()))).await
}

#[tauri::command]
pub async fn backups_revert_entry(
    state: State<'_, EngineState>,
    session: Uuid,
    entry: Uuid,
) -> AppResult<()> {
    blocking(state, move |e| {
        Ok(e.revert_entry(EntryRef { session, entry })?)
    })
    .await
}

/// Opens Windows' own System Restore wizard. Apex Tune never starts a
/// system restore by itself: it reboots the PC and you should choose it.
#[tauri::command]
pub fn open_system_restore() -> AppResult<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("rstrui.exe").spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    Err(AppError::Other(
        "System Restore is a Windows feature.".into(),
    ))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    #[serde(flatten)]
    pub profile: Profile,
    /// How many of its tweaks are on right now.
    pub applied: usize,
}

#[tauri::command]
pub async fn profiles_list(state: State<'_, EngineState>) -> AppResult<Vec<ProfileView>> {
    blocking(state, |e| {
        let on: Vec<String> = e
            .catalog()
            .iter()
            .filter(|t| matches!(t.detect(e.ctx()), Ok(TweakState::Applied)))
            .map(|t| t.id().to_owned())
            .collect();
        Ok(profiles::built_ins(e)
            .into_iter()
            .chain(profiles::saved())
            .map(|p| ProfileView {
                applied: p.tweaks.iter().filter(|t| on.contains(t)).count(),
                profile: p,
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn profile_save(state: State<'_, EngineState>, name: String) -> AppResult<Profile> {
    if name.trim().is_empty() {
        return Err(AppError::Other("Give the profile a name.".into()));
    }
    blocking(state, move |e| Ok(profiles::save_current(e, &name)?)).await
}

/// Applies a profile's tweaks. With `exclusive`, tweaks Apex Tune turned on
/// that aren't in the profile are reverted too.
#[tauri::command]
pub async fn profile_apply(
    app: AppHandle,
    state: State<'_, EngineState>,
    id: String,
    exclusive: bool,
) -> AppResult<BatchReport> {
    blocking(state, move |e| {
        let profile = profiles::find(e, &id)
            .ok_or_else(|| AppError::Other("That profile no longer exists.".into()))?;
        let mut report = preset::apply_batch(e, &profile.tweaks, |r| {
            let _ = app.emit(PRESET_PROGRESS_EVENT, r);
        })?;
        if exclusive {
            for tid in profiles::to_turn_off(e, &profile) {
                let Some(tweak) = e.tweak(&tid) else { continue };
                let result = match e.revert(&*tweak) {
                    Ok(()) => {
                        report.restart = report.restart.max(tweak.requires_restart());
                        BatchResult {
                            id: tid,
                            name: tweak.name().into(),
                            status: BatchStatus::Applied,
                            error_kind: None,
                            error: None,
                        }
                    }
                    Err(err) => BatchResult {
                        id: tid,
                        name: tweak.name().into(),
                        status: BatchStatus::Failed,
                        error_kind: Some(err.kind()),
                        error: Some(err.to_string()),
                    },
                };
                let _ = app.emit(PRESET_PROGRESS_EVENT, &result);
                report.results.push(result);
            }
        }
        tracing::info!(target: crate::log::ACTIVITY_TARGET, "Applied profile \"{}\"", profile.name);
        Ok(report)
    })
    .await
}

#[tauri::command]
pub fn profile_delete(id: String) -> AppResult<()> {
    Ok(profiles::delete(&id)?)
}

#[tauri::command]
pub async fn profile_export(
    state: State<'_, EngineState>,
    id: String,
    path: String,
) -> AppResult<()> {
    blocking(state, move |e| {
        let profile = profiles::find(e, &id)
            .ok_or_else(|| AppError::Other("That profile no longer exists.".into()))?;
        Ok(profiles::write(&profile, std::path::Path::new(&path))?)
    })
    .await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub profile: Profile,
    /// Tweak ids this version doesn't know, left out.
    pub dropped: Vec<String>,
}

#[tauri::command]
pub async fn profile_import(state: State<'_, EngineState>, path: String) -> AppResult<Imported> {
    blocking(state, move |e| {
        let (profile, dropped) =
            profiles::import(e, std::path::Path::new(&path)).map_err(AppError::Other)?;
        Ok(Imported { profile, dropped })
    })
    .await
}
