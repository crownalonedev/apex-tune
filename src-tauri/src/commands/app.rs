use std::sync::Arc;

use serde::Serialize;
use sysinfo::System;
use tauri::State;

use crate::log::{ActivityEntry, ActivityLog};
use crate::{paths, platform};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub elevated: bool,
    pub os_name: String,
    pub os_build: String,
    pub data_dir: String,
}

pub fn app_info_now() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        elevated: platform::is_elevated(),
        os_name: System::long_os_version().unwrap_or_else(|| "Unknown OS".into()),
        os_build: System::kernel_version().unwrap_or_default(),
        data_dir: paths::data_root().display().to_string(),
    }
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    app_info_now()
}

#[tauri::command]
pub fn activity_recent(log: State<'_, Arc<ActivityLog>>) -> Vec<ActivityEntry> {
    log.recent()
}
