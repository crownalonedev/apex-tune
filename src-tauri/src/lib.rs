pub mod commands;
pub mod error;
pub mod log;
pub mod paths;
pub mod platform;

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::activity_recent,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Apex Tune");
}
