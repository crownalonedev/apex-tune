//! Telemetry and consumer scheduled tasks, and a conservative set of
//! services that nothing on a gaming PC needs.

use serde::Serialize;

use crate::sys::{StartType, SysCtx, SysResult};
use crate::tweaks::Risk;
use crate::tweaks::atom::Atom;

struct TaskSpec {
    path: &'static str,
    what: &'static str,
}

const TASKS: &[TaskSpec] = &[
    TaskSpec {
        path: r"\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser",
        what: "Scans installed programs and sends compatibility telemetry. Known for CPU and disk spikes.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser Exp",
        what: "Newer variant of the compatibility telemetry scan.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Application Experience\ProgramDataUpdater",
        what: "Collects program telemetry for the Customer Experience Improvement Program.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Application Experience\PcaPatchDbTask",
        what: "Updates the Program Compatibility Assistant database.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Customer Experience Improvement Program\Consolidator",
        what: "Gathers and sends usage data for the Customer Experience Improvement Program.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip",
        what: "Collects USB usage statistics.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Autochk\Proxy",
        what: "Collects and uploads disk-check (autochk) data.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector",
        what: "Sends disk diagnostic information to Microsoft.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Feedback\Siuf\DmClient",
        what: "Feedback and survey data.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Feedback\Siuf\DmClientOnScenarioDownload",
        what: "Feedback and survey data.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Maps\MapsUpdateTask",
        what: "Checks for offline map updates.",
    },
    TaskSpec {
        path: r"\Microsoft\Windows\Maps\MapsToastTask",
        what: "Map download notifications.",
    },
];

struct ServiceSpec {
    name: &'static str,
    display: &'static str,
    what: &'static str,
    start: StartType,
    stop: bool,
    risk: Risk,
}

const SERVICES: &[ServiceSpec] = &[
    ServiceSpec {
        name: "DiagTrack",
        display: "Connected User Experiences and Telemetry",
        what: "Sends diagnostic data to Microsoft in the background. Windows Update keeps working without it.",
        start: StartType::Disabled,
        stop: true,
        risk: Risk::Moderate,
    },
    ServiceSpec {
        name: "MapsBroker",
        display: "Downloaded Maps Manager",
        what: "Starts automatically for offline maps. Set to Manual it starts only if an app needs it.",
        start: StartType::Manual,
        stop: false,
        risk: Risk::Safe,
    },
    ServiceSpec {
        name: "Fax",
        display: "Fax",
        what: "Sending and receiving faxes.",
        start: StartType::Disabled,
        stop: true,
        risk: Risk::Safe,
    },
    ServiceSpec {
        name: "RetailDemo",
        display: "Retail Demo",
        what: "Store demo mode for PCs on display in shops.",
        start: StartType::Disabled,
        stop: true,
        risk: Risk::Safe,
    },
    ServiceSpec {
        name: "wisvc",
        display: "Windows Insider Service",
        what: "Only used by the Windows Insider preview program.",
        start: StartType::Disabled,
        stop: true,
        risk: Risk::Safe,
    },
    ServiceSpec {
        name: "Spooler",
        display: "Print Spooler",
        what: "Printing. Only turn this off if you never print or save to PDF through a printer.",
        start: StartType::Disabled,
        stop: true,
        risk: Risk::Moderate,
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub path: String,
    pub name: String,
    pub description: String,
    /// `None` when the task isn't on this PC.
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRow {
    pub name: String,
    pub display: String,
    pub description: String,
    pub current: Option<StartType>,
    pub running: bool,
    pub recommended: StartType,
    pub risk: Risk,
}

pub fn tasks(ctx: &SysCtx) -> SysResult<Vec<TaskRow>> {
    TASKS
        .iter()
        .map(|t| {
            Ok(TaskRow {
                path: t.path.into(),
                name: t.path.rsplit('\\').next().unwrap_or(t.path).into(),
                description: t.what.into(),
                enabled: ctx.tasks.get(t.path)?.map(|(_, enabled)| enabled),
            })
        })
        .collect()
}

pub fn services(ctx: &SysCtx) -> SysResult<Vec<ServiceRow>> {
    SERVICES
        .iter()
        .map(|s| {
            let state = ctx.services.query(s.name)?;
            Ok(ServiceRow {
                name: s.name.into(),
                display: s.display.into(),
                description: s.what.into(),
                current: state.map(|st| st.start),
                running: state.is_some_and(|st| st.running),
                recommended: s.start,
                risk: s.risk,
            })
        })
        .collect()
}

pub fn task_atom(path: &str) -> Option<Atom> {
    TASKS
        .iter()
        .find(|t| t.path.eq_ignore_ascii_case(path))
        .map(|t| Atom::TaskEnabled {
            path: t.path.into(),
            enabled: false,
        })
}

pub fn service_atom(name: &str) -> Option<Atom> {
    SERVICES
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .map(|s| Atom::service(s.name, s.start, s.stop))
}

pub fn task_action(path: &str) -> String {
    format!("task.{path}")
}

pub fn service_action(name: &str) -> String {
    format!("service.{name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::Engine;

    #[test]
    fn every_catalog_entry_passes_the_guard() {
        for t in TASKS {
            crate::guard::check_atom(&task_atom(t.path).unwrap()).unwrap();
        }
        for s in SERVICES {
            crate::guard::check_atom(&service_atom(s.name).unwrap()).unwrap();
        }
    }

    #[test]
    fn disabling_a_task_and_a_service_round_trips() {
        let sys = FakeSystem::new();
        let path = TASKS[0].path;
        sys.tasks.add(path, true);
        sys.services
            .install("DiagTrack", StartType::Automatic, true);
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);

        engine
            .apply_action(&task_action(path), "x", vec![task_atom(path).unwrap()])
            .unwrap();
        engine
            .apply_action(
                &service_action("DiagTrack"),
                "y",
                vec![service_atom("DiagTrack").unwrap()],
            )
            .unwrap();
        assert_eq!(sys.tasks.enabled(path), Some(false));
        assert_eq!(
            sys.services.state("DiagTrack").unwrap().start,
            StartType::Disabled
        );
        assert_eq!(tasks(&sys.ctx).unwrap()[0].enabled, Some(false));
        assert!(
            tasks(&sys.ctx).unwrap()[1].enabled.is_none(),
            "missing tasks are reported, not errors"
        );

        engine.revert_everything();
        assert_eq!(sys.tasks.enabled(path), Some(true));
        let st = sys.services.state("DiagTrack").unwrap();
        assert_eq!((st.start, st.running), (StartType::Automatic, true));
    }

    #[test]
    fn a_protected_task_failure_is_isolated() {
        let sys = FakeSystem::new();
        sys.tasks.add(TASKS[0].path, true);
        sys.tasks.add(TASKS[1].path, true);
        sys.tasks.fail.lock().unwrap().push(TASKS[0].path.into());
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        assert!(
            engine
                .apply_action(
                    &task_action(TASKS[0].path),
                    "a",
                    vec![task_atom(TASKS[0].path).unwrap()]
                )
                .is_err()
        );
        engine
            .apply_action(
                &task_action(TASKS[1].path),
                "b",
                vec![task_atom(TASKS[1].path).unwrap()],
            )
            .unwrap();
        assert_eq!(sys.tasks.enabled(TASKS[1].path), Some(false));
    }
}
