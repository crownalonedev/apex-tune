//! Timer resolution: reading it, the benchmark, the tray helper that holds
//! it, its sign-in task, and the Windows 11 global-requests switch.

pub mod benchmark;
pub mod helper;

use serde::{Deserialize, Serialize};

use crate::hw::windows_version;
use crate::sys::{Hive, SysCtx, SysResult};
use crate::tweaks::atom::Atom;
use crate::tweaks::{Applicability, Category, Restart, Risk, Tweak, TweakMeta};

pub const AUTOSTART_ACTION: &str = "timer.autostart";
pub const AUTOSTART_TASK: &str = r"\ApexTune\Timer";
pub const DEFAULT_UNITS: u32 = 5_000;

const KERNEL_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel";
/// Windows Server 2022 / Windows 11 are the first builds that honour the switch.
const FIRST_GLOBAL_REQUESTS_BUILD: u32 = 20_348;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionMs {
    pub coarsest: f64,
    pub finest: f64,
    pub current: f64,
}

#[cfg(windows)]
pub fn current() -> Option<ResolutionMs> {
    apex_timer::query().map(|r| ResolutionMs {
        coarsest: apex_timer::to_ms(r.coarsest),
        finest: apex_timer::to_ms(r.finest),
        current: apex_timer::to_ms(r.current),
    })
}

#[cfg(not(windows))]
pub fn current() -> Option<ResolutionMs> {
    None
}

/// The resolution the user last chose, remembered between sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerConfig {
    pub units: u32,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            units: DEFAULT_UNITS,
        }
    }
}

fn config_path() -> std::path::PathBuf {
    crate::paths::data_root().join("timer.json")
}

pub fn load_config() -> TimerConfig {
    std::fs::read(config_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_config(config: TimerConfig) -> std::io::Result<()> {
    std::fs::write(config_path(), serde_json::to_vec_pretty(&config)?)
}

/// The sign-in task that starts the helper with this resolution.
pub fn autostart_atoms(helper: &std::path::Path, units: u32) -> Vec<Atom> {
    vec![Atom::LogonTask {
        path: AUTOSTART_TASK.into(),
        command: helper.display().to_string(),
        arguments: format!("--resolution {units}"),
        description: format!(
            "Holds a {} ms timer resolution for Apex Tune.",
            apex_timer::to_ms(units)
        ),
        highest: false,
    }]
}

pub struct GlobalTimerRequests {
    meta: TweakMeta,
}

impl GlobalTimerRequests {
    pub fn new() -> Self {
        Self {
            meta: TweakMeta {
                id: "global_timer_requests",
                name: "System-wide timer requests",
                description: "Since Windows 10 2004, a timer resolution request only affects the program that \
                              makes it. This switch makes Windows 11 apply the finest request system-wide again, so \
                              the timer helper speeds up Sleep() in games too. Slightly raises idle power use.",
                category: Category::Timer,
                risk: Risk::Advanced,
                restart: Restart::Reboot,
                recommended: false,
            },
        }
    }
}

impl Default for GlobalTimerRequests {
    fn default() -> Self {
        Self::new()
    }
}

impl Tweak for GlobalTimerRequests {
    fn meta(&self) -> &TweakMeta {
        &self.meta
    }

    fn applicable(&self, ctx: &SysCtx) -> SysResult<Applicability> {
        Ok(match windows_version::read(ctx)? {
            Some(v) if v.build < FIRST_GLOBAL_REQUESTS_BUILD => {
                Applicability::NotApplicable("Needs Windows 11 or Windows Server 2022.".into())
            }
            _ => Applicability::Applicable,
        })
    }

    fn atoms(&self, _ctx: &SysCtx) -> SysResult<Vec<Atom>> {
        Ok(vec![Atom::reg_dword(
            Hive::LocalMachine,
            KERNEL_KEY,
            "GlobalTimerResolutionRequests",
            1,
        )])
    }
}

#[cfg(windows)]
pub mod win_backend {
    use std::time::Instant;

    use windows::Win32::System::Threading::Sleep;

    use super::benchmark::TimerBackend;

    /// Measures the real `Sleep` that games use. Rust's own sleep uses a
    /// high-resolution waitable timer that ignores timer resolution.
    pub struct WinTimer;

    impl TimerBackend for WinTimer {
        fn finest(&self) -> Option<u32> {
            apex_timer::query().map(|r| r.finest)
        }
        fn request(&mut self, units: u32) -> Option<u32> {
            apex_timer::request(units)
        }
        fn release(&mut self, units: u32) {
            apex_timer::release(units);
        }
        fn sleep_one_ms(&mut self) -> f64 {
            let start = Instant::now();
            // SAFETY: plain Win32 sleep.
            unsafe { Sleep(1) };
            start.elapsed().as_secs_f64() * 1000.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::RegData;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::TweakState;

    #[test]
    fn global_requests_not_applicable_on_windows_10() {
        let sys = FakeSystem::new();
        let cv = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        sys.registry.seed(
            Hive::LocalMachine,
            cv,
            "CurrentBuildNumber",
            RegData::Sz("19045".into()),
        );
        let tweak = GlobalTimerRequests::new();
        assert!(matches!(
            tweak.detect(&sys.ctx).unwrap(),
            TweakState::NotApplicable(_)
        ));
        sys.registry.seed(
            Hive::LocalMachine,
            cv,
            "CurrentBuildNumber",
            RegData::Sz("26100".into()),
        );
        assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::NotApplied);
    }

    #[test]
    fn autostart_is_reversible() {
        use crate::backup::journal::Journal;
        use crate::tweaks::Engine;
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        let helper = std::path::Path::new(r"C:\Program Files\Apex Tune\apex-timer.exe");
        engine
            .apply_action(
                AUTOSTART_ACTION,
                "Start timer helper at sign-in",
                autostart_atoms(helper, 5_000),
            )
            .unwrap();
        let (xml, enabled) = sys.ctx.tasks.get(AUTOSTART_TASK).unwrap().unwrap();
        assert!(enabled && xml.contains("--resolution 5000") && xml.contains(r"PC\gamer"));

        // Changing the resolution re-registers; reverting goes back step by step.
        engine
            .apply_action(
                AUTOSTART_ACTION,
                "Start timer helper at sign-in",
                autostart_atoms(helper, 10_000),
            )
            .unwrap();
        assert!(
            sys.ctx
                .tasks
                .get(AUTOSTART_TASK)
                .unwrap()
                .unwrap()
                .0
                .contains("--resolution 10000")
        );
        let report = engine.revert_everything();
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert!(sys.ctx.tasks.get(AUTOSTART_TASK).unwrap().is_none());
    }

    #[test]
    fn config_defaults_to_half_a_millisecond() {
        assert_eq!(TimerConfig::default().units, 5_000);
    }
}
