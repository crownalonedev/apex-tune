//! The backup history as the UI shows it: sessions, each change, and what
//! it changed from and to ("What changed?").

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::journal::{Action, EntryRef, Journal, Status};
use crate::sys::{RegData, RestorePoint};
use crate::tweaks::atom::{Atom, Original, RegTarget};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeView {
    pub target: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub session: Uuid,
    pub entry: Uuid,
    pub name: String,
    pub action: Action,
    pub status: Status,
    pub at: DateTime<Utc>,
    pub error: Option<String>,
    /// Still in effect and can be reverted on its own.
    pub revertible: bool,
    pub reverted: bool,
    pub changes: Vec<ChangeView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub restore_point: Option<RestorePoint>,
    pub restore_point_skipped: bool,
    pub entries: Vec<EntryView>,
}

fn reg(data: &Option<RegData>) -> String {
    data.as_ref()
        .map_or("(not set)".into(), ToString::to_string)
}

fn list(l: &[String]) -> String {
    if l.is_empty() {
        "Automatic".into()
    } else {
        l.join(", ")
    }
}

/// What a change set things to.
pub fn after_text(atom: &Atom) -> String {
    match atom {
        Atom::Registry { target, .. } => match target {
            RegTarget::Set(d) => d.to_string(),
            RegTarget::Absent => "(removed)".into(),
        },
        Atom::Service { start, stop, .. } => {
            if *stop {
                format!("{start}, stopped")
            } else {
                start.to_string()
            }
        }
        Atom::PowerPlan { .. } => "Created".into(),
        Atom::PowerSetting { unit, value, .. } => unit.format(*value),
        Atom::ActivePowerPlan { guid } => guid.clone(),
        Atom::LogonTask {
            command, arguments, ..
        } => format!("{command} {arguments}"),
        Atom::TaskEnabled { enabled, .. } => if *enabled { "Enabled" } else { "Disabled" }.into(),
        Atom::AppRemoved { .. } => "Removed".into(),
        Atom::Spi { value, .. } => if *value { "On" } else { "Off" }.into(),
        Atom::DisplayRefresh { hz, .. } => format!("{hz} Hz"),
        Atom::Dns { v4, .. } => list(v4),
        Atom::NvAppProfile { .. } => "Created".into(),
        Atom::NvSetting { value_label, .. } => value_label.clone(),
    }
}

/// What was there before.
pub fn before_text(atom: &Atom, original: &Original) -> String {
    match (atom, original) {
        (_, Original::Registry { value, .. }) => reg(value),
        (_, Original::Service { state }) => state.map_or("Not installed".into(), |s| {
            format!("{}{}", s.start, if s.running { ", running" } else { "" })
        }),
        (_, Original::PowerPlan { existed }) => {
            if *existed { "Existed" } else { "Didn't exist" }.into()
        }
        (Atom::PowerSetting { unit, .. }, Original::PowerSetting { value }) => {
            value.map_or("(plan didn't exist)".into(), |v| unit.format(v))
        }
        (_, Original::PowerSetting { value }) => value.map_or("(unset)".into(), |v| v.to_string()),
        (_, Original::ActivePowerPlan { guid }) => guid.clone(),
        (_, Original::LogonTask { xml, .. }) => if xml.is_some() {
            "Existed"
        } else {
            "Not set up"
        }
        .into(),
        (_, Original::TaskEnabled { enabled }) => match enabled {
            None => "Not present".into(),
            Some(true) => "Enabled".into(),
            Some(false) => "Disabled".into(),
        },
        (_, Original::AppRemoved { installed }) => if *installed {
            "Installed"
        } else {
            "Not installed"
        }
        .into(),
        (_, Original::Spi { value }) => if *value { "On" } else { "Off" }.into(),
        (_, Original::DisplayRefresh { hz }) => format!("{hz} Hz"),
        (_, Original::Dns { v4, .. }) => list(v4),
        (_, Original::NvAppProfile { existed }) => if *existed { "Existed" } else { "None" }.into(),
        (_, Original::NvSetting { value }) => {
            value.map_or("Driver default".into(), |v| format!("0x{v:X}"))
        }
    }
}

pub fn sessions(journal: &Journal) -> Vec<SessionView> {
    let active: Vec<EntryRef> = journal.active_applies().map(|(r, _)| r).collect();
    journal
        .sessions()
        .rev()
        .filter(|s| {
            s.entries.iter().any(|e| e.action != Action::System) || s.restore_point.is_some()
        })
        .map(|s| SessionView {
            id: s.id,
            started_at: s.started_at,
            restore_point: s.restore_point.clone(),
            restore_point_skipped: s.restore_point_skipped,
            entries: s
                .entries
                .iter()
                .rev()
                .filter(|e| e.action != Action::System)
                .map(|e| {
                    let r = EntryRef {
                        session: s.id,
                        entry: e.id,
                    };
                    EntryView {
                        session: s.id,
                        entry: e.id,
                        name: e.tweak_name.clone(),
                        action: e.action,
                        status: e.status,
                        at: e.started_at,
                        error: e.error.clone(),
                        revertible: active.contains(&r),
                        reverted: e.reverted_by.is_some(),
                        changes: e
                            .records
                            .iter()
                            .map(|rec| ChangeView {
                                target: rec.atom.label(),
                                before: before_text(&rec.atom, &rec.before),
                                after: if e.action == Action::Revert {
                                    "Restored".into()
                                } else {
                                    after_text(&rec.atom)
                                },
                            })
                            .collect(),
                    }
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::Hive;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::Engine;
    use crate::tweaks::test_support::*;

    #[test]
    fn shows_each_change_with_before_and_after_and_hides_bookkeeping() {
        let sys = FakeSystem::new();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(5));
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        engine.apply(&two_values()).unwrap();

        let view = sessions(engine.journal());
        assert_eq!(view.len(), 1);
        assert!(view[0].restore_point.is_some());
        let entry = &view[0].entries[0];
        assert!(entry.revertible);
        assert_eq!(entry.changes.len(), 2);
        assert_eq!(entry.changes[0].before, "5 (0x5)");
        assert_eq!(entry.changes[0].after, "1 (0x1)");
        assert_eq!(entry.changes[1].before, "(not set)");
        assert!(
            view[0].entries.iter().all(|e| e.action != Action::System),
            "restore-point bookkeeping hidden"
        );
    }
}
