pub mod backup;
pub mod commands;
pub mod debloat;
pub mod error;
pub mod guard;
pub mod hw;
pub mod log;
pub mod paths;
pub mod platform;
pub mod power;
pub mod sys;
pub mod timer;
pub mod tweaks;

use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::log::{ACTIVITY_TARGET, ActivityLog};

pub const ACTIVITY_EVENT: &str = "activity://entry";
const ACTIVITY_CAPACITY: usize = 500;

pub fn run() {
    if let Err(e) = paths::ensure_dirs() {
        // Logging isn't up yet; stderr is all we have.
        eprintln!(
            "Apex Tune could not prepare {}: {e}",
            paths::data_root().display()
        );
    }

    let activity = Arc::new(ActivityLog::new(ACTIVITY_CAPACITY));
    let log_guard = match log::init(&paths::logs_dir(), activity.clone()) {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("Apex Tune logging disabled: {e}");
            None
        }
    };

    tauri::Builder::default()
        // Must be registered first so a second launch focuses this window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .manage(activity.clone())
        .manage(commands::dashboard::HardwareCache::default())
        .manage(hw::stats::Sampler::default())
        .setup(move |app| {
            let handle = app.handle().clone();
            activity.set_sink(move |entry| {
                let _ = handle.emit(ACTIVITY_EVENT, entry);
            });
            // Kept for the life of the app so buffered log lines are flushed on exit.
            app.manage(log_guard);

            let info = commands::app::app_info_now();
            tracing::info!(target: ACTIVITY_TARGET, "Apex Tune {} started on {}", info.version, info.os_name);
            if !info.elevated && cfg!(windows) {
                tracing::warn!("Apex Tune is not running as administrator; changes will fail");
            }
            app.manage(start_engine());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::activity_recent,
            commands::safety::safety_status,
            commands::safety::create_restore_point,
            commands::safety::enable_system_protection,
            commands::safety::skip_restore_point,
            commands::safety::revert_everything,
            commands::safety::list_restore_points,
            commands::safety::take_recovery_messages,
            commands::dashboard::hardware_summary,
            commands::dashboard::stats_subscribe,
            commands::dashboard::stats_unsubscribe,
            commands::tweaks::tweaks_overview,
            commands::tweaks::apply_tweak,
            commands::tweaks::revert_tweak,
            commands::tweaks::preset_preview,
            commands::tweaks::tweak_plan,
            commands::tweaks::apply_preset,
            commands::power::power_overview,
            commands::power::power_switch,
            commands::timer::timer_status,
            commands::timer::timer_start,
            commands::timer::timer_stop,
            commands::timer::timer_set_autostart,
            commands::timer::timer_benchmark,
            commands::debloat::debloat_apps,
            commands::debloat::debloat_remove_apps,
            commands::debloat::debloat_restore_app,
            commands::debloat::debloat_startup,
            commands::debloat::debloat_set_startup,
            commands::debloat::debloat_background,
            commands::debloat::debloat_apply_background,
            commands::debloat::debloat_restore_background,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Apex Tune");
}

/// Opens the journal and rolls back anything a crash left half-done. If the
/// backups folder can't be read, the engine stays off and changes are refused.
fn start_engine() -> commands::safety::EngineState {
    match backup::journal::Journal::open(&paths::backups_dir()) {
        Ok(journal) => {
            let mut engine = tweaks::Engine::new(sys::SysCtx::native(), journal, tweaks::catalog());
            let recovery = engine.recover();
            commands::safety::EngineState::new(Ok(engine), recovery)
        }
        Err(e) => {
            tracing::error!(target: ACTIVITY_TARGET, "Backups folder unavailable, changes are disabled: {e}");
            commands::safety::EngineState::new(Err(e.to_string()), Vec::new())
        }
    }
}
