use std::sync::{Arc, Mutex};

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::sys::RestorePoint;
use crate::tweaks::Engine;
use crate::tweaks::engine::{RevertReport, SafetyStatus};

/// The tweak engine, or why it couldn't start. Without a working backups
/// folder no change is allowed, so every command reports that reason.
#[derive(Clone)]
pub struct EngineState(Arc<Inner>);

struct Inner {
    engine: Result<Mutex<Engine>, String>,
    /// What startup recovery did, shown once by the UI.
    recovery: Mutex<Vec<String>>,
}

impl EngineState {
    pub fn new(engine: Result<Engine, String>, recovery: Vec<String>) -> Self {
        Self(Arc::new(Inner {
            engine: engine.map(Mutex::new),
            recovery: Mutex::new(recovery),
        }))
    }

    pub fn with<T>(&self, f: impl FnOnce(&mut Engine) -> AppResult<T>) -> AppResult<T> {
        let mutex = self.0.engine.as_ref().map_err(|reason| {
            AppError::Other(format!(
                "Changes are disabled because backups can't be saved: {reason}"
            ))
        })?;
        let mut engine = mutex.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut engine)
    }
}

/// Runs engine work off the async runtime: registry, SCM and WMI calls block.
pub(crate) async fn blocking<T: Send + 'static>(
    state: State<'_, EngineState>,
    f: impl FnOnce(&mut Engine) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.with(f))
        .await
        .map_err(|e| AppError::Other(format!("Background task failed: {e}")))?
}

#[tauri::command]
pub async fn safety_status(state: State<'_, EngineState>) -> AppResult<SafetyStatus> {
    blocking(state, |e| Ok(e.safety_status())).await
}

#[tauri::command]
pub async fn create_restore_point(state: State<'_, EngineState>) -> AppResult<SafetyStatus> {
    blocking(state, |e| {
        e.ensure_restore_point()?;
        Ok(e.safety_status())
    })
    .await
}

#[tauri::command]
pub async fn enable_system_protection(state: State<'_, EngineState>) -> AppResult<SafetyStatus> {
    blocking(state, |e| {
        e.enable_protection()?;
        Ok(e.safety_status())
    })
    .await
}

#[tauri::command]
pub async fn skip_restore_point(state: State<'_, EngineState>) -> AppResult<SafetyStatus> {
    blocking(state, |e| {
        e.skip_restore_point()?;
        Ok(e.safety_status())
    })
    .await
}

#[tauri::command]
pub async fn revert_everything(state: State<'_, EngineState>) -> AppResult<RevertReport> {
    blocking(state, |e| {
        let report = e.revert_everything();
        // The timer helper isn't a stored setting, but "everything" includes it.
        if let Err(err) = crate::timer::helper::stop() {
            tracing::warn!("Couldn't stop the timer helper: {err}");
        }
        Ok(report)
    })
    .await
}

#[tauri::command]
pub async fn list_restore_points(state: State<'_, EngineState>) -> AppResult<Vec<RestorePoint>> {
    blocking(state, |e| {
        Ok(e.ctx()
            .restore_points
            .list()
            .map_err(crate::tweaks::TweakError::from)?)
    })
    .await
}

/// Messages from rolling back changes interrupted last time. Returned once.
#[tauri::command]
pub fn take_recovery_messages(state: State<'_, EngineState>) -> Vec<String> {
    std::mem::take(&mut *state.0.recovery.lock().unwrap_or_else(|p| p.into_inner()))
}
