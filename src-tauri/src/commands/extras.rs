//! Games, DNS, Windows Update pause, storage cleanup and System Check.

use serde::Serialize;
use tauri::State;

use super::safety::{EngineState, blocking};
use crate::actions::{self, AdapterDns, DnsPreset};
use crate::error::{AppError, AppResult};
use crate::games::{self, Collector, Game, JvmAdvice};
use crate::insights::{self, Finding, Fix};
use crate::log::ACTIVITY_TARGET;
use crate::storage::{self, CleanReport, Target, TargetInfo};
use crate::tweaks::atom::Atom;
use crate::tweaks::{Engine, TweakError};

fn sys<T>(r: crate::sys::SysResult<T>) -> AppResult<T> {
    r.map_err(|e| AppError::from(TweakError::from(e)))
}

/// Turns an action on (journaled apply) or off (reverts Apex Tune's change).
fn set_action(
    e: &mut Engine,
    id: &str,
    name: &str,
    on: bool,
    atoms: impl FnOnce(&Engine) -> AppResult<Vec<Atom>>,
) -> AppResult<()> {
    if on {
        let atoms = atoms(e)?;
        e.apply_action(id, name, atoms)?;
        return Ok(());
    }
    let ours = e
        .journal()
        .active_applies()
        .any(|(_, entry)| entry.tweak_id == id);
    if !ours {
        return Err(AppError::Other(
            "Apex Tune didn't make this change, so there's nothing to undo.".into(),
        ));
    }
    Ok(e.revert_action(id)?)
}

// ---- Games -------------------------------------------------------------------------

#[tauri::command]
pub async fn games_list(state: State<'_, EngineState>) -> AppResult<Vec<Game>> {
    blocking(state, |e| sys(games::list(e.ctx()))).await
}

#[tauri::command]
pub async fn game_set_priority(
    state: State<'_, EngineState>,
    exe: String,
    on: bool,
) -> AppResult<()> {
    blocking(state, move |e| {
        set_action(
            e,
            &games::priority_action(&exe),
            &format!("High CPU priority for {exe}"),
            on,
            |_| Ok(vec![games::priority_atom(&exe)]),
        )
    })
    .await
}

#[tauri::command]
pub async fn game_set_fso(state: State<'_, EngineState>, path: String, on: bool) -> AppResult<()> {
    blocking(state, move |e| {
        let name = format!("Fullscreen optimizations off for {path}");
        let p = path.clone();
        set_action(e, &games::fso_action(&path), &name, on, move |e| {
            Ok(vec![sys(games::fso_atom(e.ctx(), &p))?])
        })
    })
    .await
}

#[tauri::command]
pub fn minecraft_jvm(modded: bool, collector: Collector) -> JvmAdvice {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    games::jvm_advice(system.total_memory(), modded, collector)
}

// ---- DNS ----------------------------------------------------------------------------

#[tauri::command]
pub async fn dns_status(state: State<'_, EngineState>) -> AppResult<Vec<AdapterDns>> {
    blocking(state, |e| sys(actions::dns_status(e.ctx()))).await
}

/// Sets DNS on one adapter, or on every adapter when `guid` is `None`.
#[tauri::command]
pub async fn dns_set(
    state: State<'_, EngineState>,
    guid: Option<String>,
    preset: DnsPreset,
) -> AppResult<Vec<AdapterDns>> {
    blocking(state, move |e| {
        let targets: Vec<String> = match guid {
            Some(g) => vec![g],
            None => sys(e.ctx().network.adapters())?
                .into_iter()
                .map(|a| a.guid)
                .collect(),
        };
        for g in targets {
            let Some(atom) = sys(actions::dns_atom(e.ctx(), &g, preset))? else {
                continue;
            };
            let label = format!("{}: {preset:?}", atom.label());
            e.apply_action(&actions::dns_action(&g), &label, vec![atom])?;
        }
        sys(actions::dns_status(e.ctx()))
    })
    .await
}

// ---- Windows Update pause -------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseStatus {
    /// ISO time updates resume, if paused.
    pub until: Option<String>,
    /// Apex Tune paused them, so it can resume them.
    pub ours: bool,
}

fn pause_status(e: &Engine) -> AppResult<PauseStatus> {
    Ok(PauseStatus {
        until: sys(actions::paused_until(e.ctx(), chrono::Utc::now()))?,
        ours: e
            .journal()
            .active_applies()
            .any(|(_, entry)| entry.tweak_id == actions::PAUSE_ACTION),
    })
}

#[tauri::command]
pub async fn updates_pause_status(state: State<'_, EngineState>) -> AppResult<PauseStatus> {
    blocking(state, |e| pause_status(e)).await
}

#[tauri::command]
pub async fn updates_pause(state: State<'_, EngineState>, weeks: u32) -> AppResult<PauseStatus> {
    blocking(state, move |e| {
        e.apply_action(
            actions::PAUSE_ACTION,
            &format!("Paused Windows Update for {weeks} weeks"),
            actions::pause_atoms(chrono::Utc::now(), weeks),
        )?;
        pause_status(e)
    })
    .await
}

#[tauri::command]
pub async fn updates_resume(state: State<'_, EngineState>) -> AppResult<PauseStatus> {
    blocking(state, |e| {
        e.revert_action(actions::PAUSE_ACTION)?;
        pause_status(e)
    })
    .await
}

// ---- Storage --------------------------------------------------------------------------

#[tauri::command]
pub async fn storage_scan() -> AppResult<Vec<TargetInfo>> {
    tauri::async_runtime::spawn_blocking(storage::scan)
        .await
        .map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub async fn storage_clean(
    state: State<'_, EngineState>,
    target: Target,
) -> AppResult<CleanReport> {
    blocking(state, move |e| {
        let report = storage::clean(e.ctx(), target).map_err(AppError::Other)?;
        tracing::info!(
            target: ACTIVITY_TARGET,
            tweak = "storage",
            "Cleaned {target:?}: freed {} MB, {} files skipped",
            report.freed_bytes / (1 << 20),
            report.skipped_files
        );
        Ok(report)
    })
    .await
}

// ---- System Check -------------------------------------------------------------------

#[tauri::command]
pub async fn insights_run(state: State<'_, EngineState>) -> AppResult<Vec<Finding>> {
    blocking(state, |e| {
        let volumes = crate::hw::detect_volumes();
        Ok(insights::run(e.ctx(), &volumes))
    })
    .await
}

#[tauri::command]
pub async fn insights_fix(state: State<'_, EngineState>, fix: FixRequest) -> AppResult<()> {
    blocking(state, move |e| match fix.into() {
        Fix::RefreshRate { device, hz } => {
            let displays = sys(e.ctx().desktop.displays())?;
            let atom = insights::refresh_atom(&displays, &device, hz)
                .ok_or_else(|| AppError::Other("That display is no longer connected.".into()))?;
            let name = format!("{}: {hz} Hz", atom.label());
            Ok(e.apply_action(&format!("display.{device}"), &name, vec![atom])?)
        }
    })
    .await
}

/// The fix as the UI sends it back.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FixRequest {
    RefreshRate { device: String, hz: u32 },
}

impl From<FixRequest> for Fix {
    fn from(f: FixRequest) -> Self {
        match f {
            FixRequest::RefreshRate { device, hz } => Fix::RefreshRate { device, hz },
        }
    }
}
