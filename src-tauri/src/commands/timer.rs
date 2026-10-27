use serde::Serialize;
#[cfg(windows)]
use tauri::Emitter;
use tauri::{AppHandle, State};

use super::safety::{EngineState, blocking};
use crate::error::{AppError, AppResult};
use crate::log::ACTIVITY_TARGET;
use crate::timer::benchmark::BenchReport;
use crate::timer::{self, AUTOSTART_ACTION, ResolutionMs, TimerConfig, helper};
use crate::tweaks::TweakError;

pub const BENCH_PROGRESS_EVENT: &str = "timer://bench";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerStatus {
    pub resolution: Option<ResolutionMs>,
    pub helper_available: bool,
    pub helper_running: bool,
    /// The resolution the helper holds (or will hold), in 100 ns units.
    pub units: u32,
    pub autostart: bool,
}

fn status(e: &crate::tweaks::Engine) -> AppResult<TimerStatus> {
    let config = timer::load_config();
    let autostart = match helper::exe_path() {
        Some(exe) => {
            let mut all = true;
            for atom in timer::autostart_atoms(&exe, config.units) {
                all &= atom.is_applied(e.ctx()).map_err(TweakError::from)?;
            }
            all
        }
        None => false,
    };
    Ok(TimerStatus {
        resolution: timer::current(),
        helper_available: helper::exe_path().is_some(),
        helper_running: helper::running(),
        units: config.units,
        autostart,
    })
}

#[tauri::command]
pub async fn timer_status(state: State<'_, EngineState>) -> AppResult<TimerStatus> {
    blocking(state, |e| status(e)).await
}

/// Starts (or restarts) the helper at this resolution and remembers it.
#[tauri::command]
pub async fn timer_start(state: State<'_, EngineState>, units: u32) -> AppResult<TimerStatus> {
    blocking(state, move |e| {
        let was_autostart = status(e)?.autostart;
        timer::save_config(TimerConfig { units })?;
        helper::start(units)?;
        tracing::info!(target: ACTIVITY_TARGET, tweak = "timer", "Holding {} ms timer resolution", apex_timer::to_ms(units));
        // Keep the sign-in task in step with the new resolution.
        if was_autostart && let Some(exe) = helper::exe_path() {
            e.apply_action(AUTOSTART_ACTION, "Start timer helper at sign-in", timer::autostart_atoms(&exe, units))?;
        }
        status(e)
    })
    .await
}

#[tauri::command]
pub async fn timer_stop(state: State<'_, EngineState>) -> AppResult<TimerStatus> {
    blocking(state, |e| {
        helper::stop()?;
        tracing::info!(target: ACTIVITY_TARGET, tweak = "timer", "Stopped holding the timer resolution");
        status(e)
    })
    .await
}

#[tauri::command]
pub async fn timer_set_autostart(
    state: State<'_, EngineState>,
    enabled: bool,
) -> AppResult<TimerStatus> {
    blocking(state, move |e| {
        if enabled {
            let exe = helper::exe_path().ok_or_else(|| {
                AppError::Other("The timer helper is missing from the Apex Tune folder.".into())
            })?;
            let units = timer::load_config().units;
            e.apply_action(
                AUTOSTART_ACTION,
                "Start timer helper at sign-in",
                timer::autostart_atoms(&exe, units),
            )?;
        } else {
            e.revert_action(AUTOSTART_ACTION)?;
        }
        status(e)
    })
    .await
}

/// Runs the Sleep(1) benchmark on its own thread; it takes about 10 seconds.
#[tauri::command]
pub async fn timer_benchmark(app: AppHandle) -> AppResult<BenchReport> {
    tauri::async_runtime::spawn_blocking(move || run_benchmark(&app))
        .await
        .map_err(|e| AppError::Other(format!("Benchmark failed: {e}")))?
}

#[cfg(windows)]
fn run_benchmark(app: &AppHandle) -> AppResult<BenchReport> {
    let mut backend = timer::win_backend::WinTimer;
    let report = timer::benchmark::run(&mut backend, 300, |done, total| {
        let _ = app.emit(BENCH_PROGRESS_EVENT, (done, total));
    });
    if let Some(units) = report.recommended {
        tracing::info!(target: ACTIVITY_TARGET, tweak = "timer", "Timer benchmark recommends {} ms", apex_timer::to_ms(units));
    }
    Ok(report)
}

#[cfg(not(windows))]
fn run_benchmark(_app: &AppHandle) -> AppResult<BenchReport> {
    Err(AppError::Other(
        "The timer benchmark only runs on Windows.".into(),
    ))
}
