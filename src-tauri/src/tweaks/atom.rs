//! Atoms: the smallest reversible changes. A declarative tweak is a list of
//! atoms; each atom can capture what's there now, write its target, restore
//! the captured original, and say whether its target is in place.

use serde::{Deserialize, Serialize};

use crate::sys::{Hive, RegData, ServiceState, StartType, SysCtx, SysResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "data", rename_all = "camelCase")]
pub enum RegTarget {
    Set(RegData),
    /// The value must not exist (e.g. removing a policy).
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Atom {
    Registry {
        hive: Hive,
        path: String,
        name: String,
        target: RegTarget,
    },
    Service {
        name: String,
        start: StartType,
        /// Also stop it now if it's running.
        stop: bool,
    },
}

/// What was there before an atom was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Original {
    Registry {
        value: Option<RegData>,
        /// Keys this change had to create, deepest first. Removed on revert if empty.
        created_keys: Vec<String>,
    },
    Service {
        /// `None` when the service isn't installed; the atom then does nothing.
        state: Option<ServiceState>,
    },
}

/// One line of a change preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePreview {
    pub target: String,
    pub from: String,
    pub to: String,
}

const ABSENT: &str = "(not set)";

impl Atom {
    pub fn reg(hive: Hive, path: &str, name: &str, data: RegData) -> Self {
        Atom::Registry {
            hive,
            path: path.into(),
            name: name.into(),
            target: RegTarget::Set(data),
        }
    }

    pub fn reg_dword(hive: Hive, path: &str, name: &str, value: u32) -> Self {
        Self::reg(hive, path, name, RegData::Dword(value))
    }

    pub fn reg_sz(hive: Hive, path: &str, name: &str, value: &str) -> Self {
        Self::reg(hive, path, name, RegData::Sz(value.into()))
    }

    pub fn reg_absent(hive: Hive, path: &str, name: &str) -> Self {
        Atom::Registry {
            hive,
            path: path.into(),
            name: name.into(),
            target: RegTarget::Absent,
        }
    }

    pub fn service(name: &str, start: StartType, stop: bool) -> Self {
        Atom::Service {
            name: name.into(),
            start,
            stop,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Atom::Registry {
                hive, path, name, ..
            } => format!(r"{hive}\{path}\{name}"),
            Atom::Service { name, .. } => format!("Service {name}"),
        }
    }

    pub fn capture(&self, ctx: &SysCtx) -> SysResult<Original> {
        match self {
            Atom::Registry {
                hive, path, name, ..
            } => Ok(Original::Registry {
                value: ctx.registry.get(*hive, path, name)?,
                created_keys: missing_keys(ctx, *hive, path)?,
            }),
            Atom::Service { name, .. } => Ok(Original::Service {
                state: ctx.services.query(name)?,
            }),
        }
    }

    pub fn write(&self, ctx: &SysCtx) -> SysResult<()> {
        match self {
            Atom::Registry {
                hive,
                path,
                name,
                target,
            } => match target {
                RegTarget::Set(data) => ctx.registry.set(*hive, path, name, data),
                RegTarget::Absent => ctx.registry.delete_value(*hive, path, name),
            },
            Atom::Service { name, start, stop } => {
                let Some(state) = ctx.services.query(name)? else {
                    return Ok(());
                };
                if state.start != *start {
                    ctx.services.set_start(name, *start)?;
                }
                if *stop && state.running {
                    ctx.services.stop(name)?;
                }
                Ok(())
            }
        }
    }

    pub fn restore(&self, ctx: &SysCtx, original: &Original) -> SysResult<()> {
        match (self, original) {
            (
                Atom::Registry {
                    hive, path, name, ..
                },
                Original::Registry {
                    value,
                    created_keys,
                },
            ) => {
                // Only touch what differs: after a failed write, the target is
                // usually unchanged and writing to it again would fail the same way.
                if ctx.registry.get(*hive, path, name)? != *value {
                    match value {
                        Some(data) => ctx.registry.set(*hive, path, name, data)?,
                        None => ctx.registry.delete_value(*hive, path, name)?,
                    }
                }
                for key in created_keys {
                    if ctx.registry.key_exists(*hive, key)? {
                        ctx.registry.delete_key_if_empty(*hive, key)?;
                    }
                }
                Ok(())
            }
            (Atom::Service { name, .. }, Original::Service { state }) => {
                let Some(before) = state else { return Ok(()) };
                let Some(now) = ctx.services.query(name)? else {
                    return Ok(());
                };
                if now.start != before.start {
                    ctx.services.set_start(name, before.start)?;
                }
                if before.running && !now.running {
                    ctx.services.start(name)?;
                }
                Ok(())
            }
            _ => Err(crate::sys::SysError::new(
                crate::sys::SysErrorKind::Other,
                "restore",
                self.label(),
                "backup record doesn't match this change",
            )),
        }
    }

    /// Whether the target state is in place. A missing service counts as in
    /// place: there's nothing to change.
    pub fn is_applied(&self, ctx: &SysCtx) -> SysResult<bool> {
        match self {
            Atom::Registry {
                hive,
                path,
                name,
                target,
            } => {
                let current = ctx.registry.get(*hive, path, name)?;
                Ok(match target {
                    RegTarget::Set(data) => current.as_ref() == Some(data),
                    RegTarget::Absent => current.is_none(),
                })
            }
            Atom::Service { name, start, .. } => {
                Ok(ctx.services.query(name)?.is_none_or(|s| s.start == *start))
            }
        }
    }

    /// Whether the current state equals a captured original (used to verify a restore).
    pub fn matches_original(&self, ctx: &SysCtx, original: &Original) -> SysResult<bool> {
        match (self, original) {
            (
                Atom::Registry {
                    hive, path, name, ..
                },
                Original::Registry { value, .. },
            ) => Ok(ctx.registry.get(*hive, path, name)? == *value),
            (Atom::Service { name, .. }, Original::Service { state }) => Ok(match state {
                None => true,
                Some(before) => ctx
                    .services
                    .query(name)?
                    .is_none_or(|now| now.start == before.start),
            }),
            _ => Ok(false),
        }
    }

    pub fn preview(&self, ctx: &SysCtx) -> SysResult<ChangePreview> {
        let target = self.label();
        match self {
            Atom::Registry {
                hive,
                path,
                name,
                target: wanted,
            } => {
                let from = ctx
                    .registry
                    .get(*hive, path, name)?
                    .map_or(ABSENT.into(), |d| d.to_string());
                let to = match wanted {
                    RegTarget::Set(data) => data.to_string(),
                    RegTarget::Absent => "(removed)".into(),
                };
                Ok(ChangePreview { target, from, to })
            }
            Atom::Service { name, start, stop } => {
                let from = ctx
                    .services
                    .query(name)?
                    .map_or("Not installed".into(), |s| s.start.to_string());
                let to = if *stop {
                    format!("{start}, stopped now")
                } else {
                    start.to_string()
                };
                Ok(ChangePreview { target, from, to })
            }
        }
    }
}

/// Keys on `path` that don't exist yet, deepest first.
fn missing_keys(ctx: &SysCtx, hive: Hive, path: &str) -> SysResult<Vec<String>> {
    let parts: Vec<&str> = path.split('\\').filter(|p| !p.is_empty()).collect();
    let mut missing = Vec::new();
    for depth in (1..=parts.len()).rev() {
        let key = parts[..depth].join("\\");
        if ctx.registry.key_exists(hive, &key)? {
            break;
        }
        missing.push(key);
    }
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::fake::FakeSystem;

    const PATH: &str = r"Software\Apex\Test";

    #[test]
    fn registry_round_trip_restores_existing_value() {
        let sys = FakeSystem::new();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "v", RegData::Dword(7));
        let atom = Atom::reg_dword(Hive::CurrentUser, PATH, "v", 1);

        let original = atom.capture(&sys.ctx).unwrap();
        atom.write(&sys.ctx).unwrap();
        assert!(atom.is_applied(&sys.ctx).unwrap());
        atom.restore(&sys.ctx, &original).unwrap();
        assert_eq!(
            sys.registry.value(Hive::CurrentUser, PATH, "v"),
            Some(RegData::Dword(7))
        );
    }

    #[test]
    fn registry_restore_removes_created_value_and_keys() {
        let sys = FakeSystem::new();
        sys.registry.seed_key(Hive::CurrentUser, "Software");
        let atom = Atom::reg_dword(Hive::CurrentUser, PATH, "v", 1);

        let original = atom.capture(&sys.ctx).unwrap();
        assert_eq!(
            original,
            Original::Registry {
                value: None,
                created_keys: vec![PATH.into(), r"Software\Apex".into()]
            }
        );
        atom.write(&sys.ctx).unwrap();
        atom.restore(&sys.ctx, &original).unwrap();
        assert!(!sys.registry.has_key(Hive::CurrentUser, r"Software\Apex"));
        assert!(sys.registry.has_key(Hive::CurrentUser, "Software"));
    }

    #[test]
    fn absent_target_deletes_and_restores_value() {
        let sys = FakeSystem::new();
        sys.registry
            .seed(Hive::LocalMachine, PATH, "p", RegData::Sz("x".into()));
        let atom = Atom::reg_absent(Hive::LocalMachine, PATH, "p");
        let original = atom.capture(&sys.ctx).unwrap();
        atom.write(&sys.ctx).unwrap();
        assert!(atom.is_applied(&sys.ctx).unwrap());
        atom.restore(&sys.ctx, &original).unwrap();
        assert_eq!(
            sys.registry.value(Hive::LocalMachine, PATH, "p"),
            Some(RegData::Sz("x".into()))
        );
    }

    #[test]
    fn service_round_trip_restores_start_type_and_running_state() {
        let sys = FakeSystem::new();
        sys.services
            .install("DiagTrack", StartType::Automatic, true);
        let atom = Atom::service("DiagTrack", StartType::Disabled, true);

        let original = atom.capture(&sys.ctx).unwrap();
        atom.write(&sys.ctx).unwrap();
        assert_eq!(
            sys.services.state("DiagTrack"),
            Some(ServiceState {
                start: StartType::Disabled,
                running: false
            })
        );
        atom.restore(&sys.ctx, &original).unwrap();
        assert_eq!(
            sys.services.state("DiagTrack"),
            Some(ServiceState {
                start: StartType::Automatic,
                running: true
            })
        );
    }

    #[test]
    fn missing_service_is_a_no_op() {
        let sys = FakeSystem::new();
        let atom = Atom::service("Fax", StartType::Disabled, false);
        let original = atom.capture(&sys.ctx).unwrap();
        atom.write(&sys.ctx).unwrap();
        assert!(atom.is_applied(&sys.ctx).unwrap());
        atom.restore(&sys.ctx, &original).unwrap();
    }

    #[test]
    fn preview_shows_before_and_after() {
        let sys = FakeSystem::new();
        let atom = Atom::reg_dword(Hive::LocalMachine, PATH, "HwSchMode", 2);
        let preview = atom.preview(&sys.ctx).unwrap();
        assert_eq!(preview.from, "(not set)");
        assert_eq!(preview.to, "2 (0x2)");
    }
}
