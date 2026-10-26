use tauri::{AppHandle, Emitter, State};

use super::safety::{EngineState, blocking};
use crate::error::{AppError, AppResult};
use crate::tweaks::engine::ApplyOutcome;
use crate::tweaks::preset::{self, BatchReport, Overview, PresetItem};

pub const PRESET_PROGRESS_EVENT: &str = "preset://progress";

#[tauri::command]
pub async fn tweaks_overview(state: State<'_, EngineState>) -> AppResult<Overview> {
    blocking(state, |e| Ok(preset::overview(e))).await
}

#[tauri::command]
pub async fn apply_tweak(state: State<'_, EngineState>, id: String) -> AppResult<ApplyOutcome> {
    blocking(state, move |e| {
        let tweak = e
            .tweak(&id)
            .ok_or_else(|| AppError::Other(format!("Unknown tweak {id}")))?;
        Ok(e.apply(&*tweak)?)
    })
    .await
}

#[tauri::command]
pub async fn revert_tweak(state: State<'_, EngineState>, id: String) -> AppResult<()> {
    blocking(state, move |e| {
        let tweak = e
            .tweak(&id)
            .ok_or_else(|| AppError::Other(format!("Unknown tweak {id}")))?;
        Ok(e.revert(&*tweak)?)
    })
    .await
}

#[tauri::command]
pub async fn preset_preview(state: State<'_, EngineState>) -> AppResult<Vec<PresetItem>> {
    blocking(state, |e| Ok(preset::preview(e)?)).await
}

/// Applies the chosen preset tweaks, emitting each result as it lands.
#[tauri::command]
pub async fn apply_preset(
    app: AppHandle,
    state: State<'_, EngineState>,
    ids: Vec<String>,
) -> AppResult<BatchReport> {
    blocking(state, move |e| {
        Ok(preset::apply_batch(e, &ids, |result| {
            let _ = app.emit(PRESET_PROGRESS_EVENT, result);
        })?)
    })
    .await
}
