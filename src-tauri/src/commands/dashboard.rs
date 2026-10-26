use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, AppResult};
use crate::hw::stats::{Sampler, StatsTick};
use crate::hw::{self, HardwareSummary};
use crate::sys::SysCtx;

pub const STATS_EVENT: &str = "stats://tick";

/// Hardware is detected once (WMI takes a moment) and cached.
#[derive(Default)]
pub struct HardwareCache(Mutex<Option<HardwareSummary>>);

#[tauri::command]
pub async fn hardware_summary(
    cache: State<'_, HardwareCache>,
    refresh: Option<bool>,
) -> AppResult<HardwareSummary> {
    if !refresh.unwrap_or(false)
        && let Some(cached) = cache.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    {
        return Ok(cached);
    }
    let summary = tauri::async_runtime::spawn_blocking(|| hw::detect(&SysCtx::native()))
        .await
        .map_err(|e| AppError::Other(format!("Hardware detection failed: {e}")))?;
    *cache.0.lock().unwrap_or_else(|p| p.into_inner()) = Some(summary.clone());
    Ok(summary)
}

#[tauri::command]
pub fn stats_subscribe(app: AppHandle, sampler: State<'_, Sampler>) {
    sampler.subscribe(Arc::new(move |tick: StatsTick| {
        let _ = app.emit(STATS_EVENT, tick);
    }));
}

#[tauri::command]
pub fn stats_unsubscribe(sampler: State<'_, Sampler>) {
    sampler.unsubscribe();
}
