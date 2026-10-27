use serde::Serialize;
use tauri::State;

use super::safety::{EngineState, blocking};
use crate::debloat::apps::{self, AppRow};
use crate::debloat::background::{self, ServiceRow, TaskRow};
use crate::debloat::startup::{self, StartupItem};
use crate::error::{AppError, AppResult};
use crate::tweaks::atom::Atom;
use crate::tweaks::{Engine, TweakError};

/// The outcome of one item in a batch. A failure never stops the others.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub id: String,
    pub ok: bool,
    pub error: Option<String>,
}

fn result(id: String, r: Result<(), TweakError>) -> ItemResult {
    match r {
        Ok(()) => ItemResult {
            id,
            ok: true,
            error: None,
        },
        Err(e) => ItemResult {
            id,
            ok: false,
            error: Some(e.to_string()),
        },
    }
}

fn sys<T>(r: crate::sys::SysResult<T>) -> AppResult<T> {
    r.map_err(|e| AppError::from(TweakError::from(e)))
}

/// Apps Apex Tune removed and can restore: (package name, display name).
fn removed_by_apex(e: &Engine) -> Vec<(String, String)> {
    e.journal()
        .active_applies()
        .flat_map(|(_, entry)| entry.records.iter())
        .filter_map(|r| match &r.atom {
            Atom::AppRemoved {
                name, display_name, ..
            } => Some((name.clone(), display_name.clone())),
            _ => None,
        })
        .collect()
}

#[tauri::command]
pub async fn debloat_apps(state: State<'_, EngineState>) -> AppResult<Vec<AppRow>> {
    blocking(state, |e| {
        let removed = removed_by_apex(e);
        sys(apps::rows(e.ctx(), &removed))
    })
    .await
}

#[tauri::command]
pub async fn debloat_remove_apps(
    state: State<'_, EngineState>,
    names: Vec<String>,
) -> AppResult<Vec<ItemResult>> {
    blocking(state, move |e| {
        e.ensure_restore_point()?;
        let mut out = Vec::new();
        for name in names {
            let r = match apps::removal(e.ctx(), &name) {
                Ok(Some(atom)) => {
                    let label = format!("Removed app {}", atom.label().trim_start_matches("App "));
                    e.apply_action(&apps::action_id(&name), &label, vec![atom])
                }
                Ok(None) => Ok(()),
                Err(err) => Err(err.into()),
            };
            out.push(result(name, r));
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn debloat_restore_app(state: State<'_, EngineState>, name: String) -> AppResult<()> {
    blocking(
        state,
        move |e| Ok(e.revert_action(&apps::action_id(&name))?),
    )
    .await
}

#[tauri::command]
pub async fn debloat_startup(state: State<'_, EngineState>) -> AppResult<Vec<StartupItem>> {
    blocking(state, |e| sys(startup::list(e.ctx()))).await
}

/// Switches a startup item. Switching back on something Apex Tune switched
/// off reverts that change instead of stacking another one.
#[tauri::command]
pub async fn debloat_set_startup(
    state: State<'_, EngineState>,
    id: String,
    enabled: bool,
) -> AppResult<()> {
    blocking(state, move |e| {
        let action = startup::action_id(&id);
        let ours = e
            .journal()
            .active_applies()
            .any(|(_, entry)| entry.tweak_id == action);
        if enabled && ours {
            return Ok(e.revert_action(&action)?);
        }
        let item = sys(startup::list(e.ctx()))?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| AppError::Other("That startup item no longer exists.".into()))?;
        if let Some(reason) = &item.protected {
            return Err(AppError::Other(format!(
                "{} can't be changed here. {reason}",
                item.name
            )));
        }
        let verb = if enabled { "on" } else { "off" };
        Ok(e.apply_action(
            &action,
            &format!("Startup {verb}: {}", item.name),
            vec![startup::toggle(&item, enabled)],
        )?)
    })
    .await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    pub tasks: Vec<TaskRow>,
    pub services: Vec<ServiceRow>,
    /// Task paths and service names Apex Tune changed, so the UI offers "Restore".
    pub changed: Vec<String>,
}

#[tauri::command]
pub async fn debloat_background(state: State<'_, EngineState>) -> AppResult<Background> {
    blocking(state, |e| {
        let changed = e
            .journal()
            .active_applies()
            .filter_map(|(_, entry)| {
                entry
                    .tweak_id
                    .strip_prefix("task.")
                    .or_else(|| entry.tweak_id.strip_prefix("service."))
                    .map(str::to_owned)
            })
            .collect();
        Ok(Background {
            tasks: sys(background::tasks(e.ctx()))?,
            services: sys(background::services(e.ctx()))?,
            changed,
        })
    })
    .await
}

/// Applies the recommendation for each listed task path or service name.
#[tauri::command]
pub async fn debloat_apply_background(
    state: State<'_, EngineState>,
    tasks: Vec<String>,
    services: Vec<String>,
) -> AppResult<Vec<ItemResult>> {
    blocking(state, move |e| {
        let mut out = Vec::new();
        for path in tasks {
            let r = match background::task_atom(&path) {
                Some(atom) => e.apply_action(
                    &background::task_action(&path),
                    &format!("Disabled task {path}"),
                    vec![atom],
                ),
                None => Err(TweakError::NotApplicable("Not on Apex Tune's list.".into())),
            };
            out.push(result(path, r));
        }
        for name in services {
            let r = match background::service_atom(&name) {
                Some(atom) => e.apply_action(
                    &background::service_action(&name),
                    &format!("Changed service {name}"),
                    vec![atom],
                ),
                None => Err(TweakError::NotApplicable("Not on Apex Tune's list.".into())),
            };
            out.push(result(name, r));
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn debloat_restore_background(
    state: State<'_, EngineState>,
    key: String,
) -> AppResult<()> {
    blocking(state, move |e| {
        let action = if key.starts_with('\\') {
            background::task_action(&key)
        } else {
            background::service_action(&key)
        };
        Ok(e.revert_action(&action)?)
    })
    .await
}
