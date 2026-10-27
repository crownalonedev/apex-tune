//! Storage cleanup. Deleting cached files can't be undone, so these are
//! one-way actions: each shows its size first, needs confirmation, skips
//! files in use, and never touches anti-cheat folders.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Serialize;

use crate::guard;
use crate::sys::SysCtx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    UserTemp,
    WindowsTemp,
    UpdateCache,
    DeliveryOptimization,
    ShaderCache,
    ErrorReports,
}

pub const ALL: [Target; 6] = [
    Target::UserTemp,
    Target::WindowsTemp,
    Target::UpdateCache,
    Target::DeliveryOptimization,
    Target::ShaderCache,
    Target::ErrorReports,
];

struct Spec {
    name: &'static str,
    description: &'static str,
    warning: Option<&'static str>,
    /// Services to stop while cleaning (always restarted afterwards).
    services: &'static [&'static str],
    /// Only delete files older than this.
    min_age: Duration,
}

fn spec(t: Target) -> Spec {
    let day = Duration::from_secs(24 * 3600);
    match t {
        Target::UserTemp => Spec {
            name: "Your temporary files",
            description: "Leftovers from installers and apps in your Temp folder. Files in use are skipped.",
            warning: None,
            services: &[],
            min_age: day,
        },
        Target::WindowsTemp => Spec {
            name: "Windows temporary files",
            description: "Windows' own Temp folder. Files newer than a day are left alone.",
            warning: None,
            services: &[],
            min_age: day,
        },
        Target::UpdateCache => Spec {
            name: "Windows Update download cache",
            description: "Update packages that are already installed. Windows Update is paused for a moment while \
                          this is cleaned and started again straight after.",
            warning: None,
            services: &["wuauserv", "bits"],
            min_age: Duration::ZERO,
        },
        Target::DeliveryOptimization => Spec {
            name: "Delivery Optimization cache",
            description: "Update pieces kept to share with other PCs.",
            warning: None,
            services: &["DoSvc"],
            min_age: Duration::ZERO,
        },
        Target::ShaderCache => Spec {
            name: "GPU shader caches",
            description: "Compiled shaders from NVIDIA, AMD and DirectX. Clearing fixes corrupted-cache stutter and \
                          frees space after driver updates.",
            warning: Some(
                "Games stutter for the first few minutes while shaders are rebuilt. Close all games first.",
            ),
            services: &[],
            min_age: Duration::ZERO,
        },
        Target::ErrorReports => Spec {
            name: "Error report archives",
            description: "Crash reports Windows already sent or queued.",
            warning: None,
            services: &[],
            min_age: Duration::ZERO,
        },
    }
}

fn env_path(var: &str, rest: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(|v| PathBuf::from(v).join(rest))
}

fn paths(t: Target) -> Vec<PathBuf> {
    let list = match t {
        Target::UserTemp => vec![std::env::var_os("TEMP").map(PathBuf::from)],
        Target::WindowsTemp => vec![env_path("SystemRoot", "Temp")],
        Target::UpdateCache => vec![env_path("SystemRoot", r"SoftwareDistribution\Download")],
        Target::DeliveryOptimization => vec![env_path(
            "SystemRoot",
            r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache",
        )],
        Target::ShaderCache => vec![
            env_path("LOCALAPPDATA", r"NVIDIA\DXCache"),
            env_path("LOCALAPPDATA", r"NVIDIA\GLCache"),
            env_path("LOCALAPPDATA", r"AMD\DxCache"),
            env_path("LOCALAPPDATA", r"AMD\GLCache"),
            env_path("LOCALAPPDATA", "D3DSCache"),
        ],
        Target::ErrorReports => vec![
            env_path("ProgramData", r"Microsoft\Windows\WER\ReportArchive"),
            env_path("ProgramData", r"Microsoft\Windows\WER\ReportQueue"),
        ],
    };
    list.into_iter().flatten().filter(|p| p.is_dir()).collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub target: Target,
    pub name: String,
    pub description: String,
    pub warning: Option<String>,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanReport {
    pub freed_bytes: u64,
    pub deleted_files: u64,
    /// Files in use or protected, left in place.
    pub skipped_files: u64,
}

fn old_enough(meta: &std::fs::Metadata, min_age: Duration) -> bool {
    min_age.is_zero()
        || meta
            .modified()
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .is_some_and(|age| age >= min_age)
}

/// Total size and count of files that cleaning would remove.
pub fn measure(dir: &Path, min_age: Duration) -> (u64, u64) {
    let mut bytes = 0;
    let mut files = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            let (b, f) = measure(&entry.path(), min_age);
            bytes += b;
            files += f;
        } else if old_enough(&meta, min_age) {
            bytes += meta.len();
            files += 1;
        }
    }
    (bytes, files)
}

/// Deletes the contents of `dir` (never `dir` itself), oldest-rule applied,
/// skipping anything that can't be deleted. Doesn't follow links.
pub fn clean_dir(dir: &Path, min_age: Duration, report: &mut CleanReport) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if guard::check_path(&path).is_err() {
            report.skipped_files += 1;
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            clean_dir(&path, min_age, report);
            // Removes the folder only if it ended up empty.
            let _ = std::fs::remove_dir(&path);
        } else if old_enough(&meta, min_age) {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    report.freed_bytes += meta.len();
                    report.deleted_files += 1;
                }
                Err(_) => report.skipped_files += 1,
            }
        }
    }
}

pub fn scan() -> Vec<TargetInfo> {
    ALL.iter()
        .map(|&t| {
            let s = spec(t);
            let (bytes, files) = paths(t)
                .iter()
                .map(|p| measure(p, s.min_age))
                .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
            TargetInfo {
                target: t,
                name: s.name.into(),
                description: s.description.into(),
                warning: s.warning.map(Into::into),
                bytes,
                files,
            }
        })
        .collect()
}

/// Executables that mean a game is running, so shader caches are in use.
const GAME_PROCESSES: &[&str] = &[
    "-shipping.exe",
    "javaw.exe",
    "minecraft.windows.exe",
    "cs2.exe",
    "r5apex.exe",
];

fn game_running() -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy().to_ascii_lowercase();
        GAME_PROCESSES.iter().any(|g| name.ends_with(g))
    })
}

pub fn clean(ctx: &SysCtx, t: Target) -> Result<CleanReport, String> {
    let s = spec(t);
    if t == Target::ShaderCache && game_running() {
        return Err("A game is running. Close it before clearing shader caches.".into());
    }
    let mut stopped = Vec::new();
    for svc in s.services {
        match ctx.services.stop(svc) {
            Ok(()) => stopped.push(*svc),
            Err(e) if e.kind == crate::sys::SysErrorKind::NotFound => {}
            Err(e) => {
                // Put back whatever was already stopped before giving up.
                for done in &stopped {
                    let _ = ctx.services.start(done);
                }
                return Err(format!("Couldn't pause {svc} to clean safely: {e}"));
            }
        }
    }
    let mut report = CleanReport::default();
    for dir in paths(t) {
        clean_dir(&dir, s.min_age, &mut report);
    }
    for svc in stopped {
        if let Err(e) = ctx.services.start(svc) {
            tracing::warn!(
                "{svc} didn't restart after cleaning; Windows will start it when needed: {e}"
            );
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_contents_but_keeps_the_root_and_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(dir.path().join("a.tmp"), vec![0u8; 100]).unwrap();
        std::fs::write(sub.join("b.tmp"), vec![0u8; 50]).unwrap();

        assert_eq!(measure(dir.path(), Duration::ZERO), (150, 2));
        assert_eq!(
            measure(dir.path(), Duration::from_secs(3600)),
            (0, 0),
            "too new"
        );

        let mut report = CleanReport::default();
        clean_dir(dir.path(), Duration::from_secs(3600), &mut report);
        assert_eq!(report.deleted_files, 0);

        clean_dir(dir.path(), Duration::ZERO, &mut report);
        assert_eq!((report.freed_bytes, report.deleted_files), (150, 2));
        assert!(dir.path().exists());
        assert!(!sub.exists(), "empty subfolders are removed");
    }

    #[test]
    fn scan_lists_every_target() {
        assert_eq!(scan().len(), ALL.len());
    }
}
