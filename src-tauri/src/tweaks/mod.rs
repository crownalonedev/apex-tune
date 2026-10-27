//! The `Tweak` trait and everything shared by tweak implementations.
//!
//! Most tweaks are declarative: they list [`Atom`]s and get `detect`,
//! `plan`, `apply` and `revert` for free. Tweaks that need custom logic
//! (DNS, AppX, power plans, NVIDIA) override those methods but still write
//! through a [`Txn`], so every change is journaled and can be rolled back.

pub mod atom;
pub mod engine;
pub mod error;
pub mod gaming;
pub mod preset;
pub mod privacy;

use serde::Serialize;

pub use atom::{Atom, ChangePreview, Original};
pub use engine::{Engine, Txn};
pub use error::TweakError;

use crate::sys::{SysCtx, SysResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    Safe,
    Moderate,
    Advanced,
}

/// What it takes for a change to take full effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Restart {
    None,
    Explorer,
    SignOut,
    Reboot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Gaming,
    Input,
    Network,
    Visual,
    Privacy,
    Startup,
    Services,
    Updates,
    Power,
    Timer,
    Nvidia,
    System,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakMeta {
    pub id: &'static str,
    pub name: &'static str,
    /// Plain English, including an honest note on how big the effect is.
    pub description: &'static str,
    pub category: Category,
    pub risk: Risk,
    pub restart: Restart,
    /// Part of the Safe "Optimize for Gaming" preset.
    pub recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "reason", rename_all = "camelCase")]
pub enum TweakState {
    Applied,
    NotApplied,
    /// Some of its settings are in place, some aren't.
    Partial,
    NotApplicable(String),
    /// Its state couldn't be read.
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applicability {
    Applicable,
    NotApplicable(String),
}

pub trait Tweak: Send + Sync {
    fn meta(&self) -> &TweakMeta;

    fn id(&self) -> &'static str {
        self.meta().id
    }
    fn name(&self) -> &'static str {
        self.meta().name
    }
    fn description(&self) -> &'static str {
        self.meta().description
    }
    fn risk(&self) -> Risk {
        self.meta().risk
    }
    fn requires_restart(&self) -> Restart {
        self.meta().restart
    }

    fn applicable(&self, _ctx: &SysCtx) -> SysResult<Applicability> {
        Ok(Applicability::Applicable)
    }

    /// The target state as atoms. Used for detection, preview and apply,
    /// and checked against the guard before anything is written.
    fn atoms(&self, ctx: &SysCtx) -> SysResult<Vec<Atom>>;

    fn detect(&self, ctx: &SysCtx) -> SysResult<TweakState> {
        if let Applicability::NotApplicable(reason) = self.applicable(ctx)? {
            return Ok(TweakState::NotApplicable(reason));
        }
        let atoms = self.atoms(ctx)?;
        let mut applied = 0;
        for atom in &atoms {
            if atom.is_applied(ctx)? {
                applied += 1;
            }
        }
        Ok(match applied {
            n if n == atoms.len() => TweakState::Applied,
            0 => TweakState::NotApplied,
            _ => TweakState::Partial,
        })
    }

    /// Exactly what `apply` would change, for the preview dialog.
    fn plan(&self, ctx: &SysCtx) -> SysResult<Vec<ChangePreview>> {
        let mut changes = Vec::new();
        for atom in self.atoms(ctx)? {
            if !atom.is_applied(ctx)? {
                changes.push(atom.preview(ctx)?);
            }
        }
        Ok(changes)
    }

    fn apply(&self, txn: &mut Txn<'_>) -> Result<(), TweakError> {
        for atom in self.atoms(txn.ctx())? {
            txn.write(atom)?;
        }
        Ok(())
    }

    /// Puts back exactly what was there before this tweak's last apply.
    fn revert(&self, txn: &mut Txn<'_>) -> Result<(), TweakError> {
        txn.restore_applied()
    }
}

/// A tweak whose atoms are fixed.
pub struct StaticTweak {
    pub meta: TweakMeta,
    pub atoms: fn() -> Vec<Atom>,
}

impl Tweak for StaticTweak {
    fn meta(&self) -> &TweakMeta {
        &self.meta
    }

    fn atoms(&self, _ctx: &SysCtx) -> SysResult<Vec<Atom>> {
        Ok((self.atoms)())
    }
}

/// Every tweak Apex Tune offers, in display order.
pub fn catalog() -> Vec<std::sync::Arc<dyn Tweak>> {
    vec![
        std::sync::Arc::new(gaming::game_mode()),
        std::sync::Arc::new(gaming::game_dvr_off()),
        std::sync::Arc::new(crate::power::GamingPlan::new()),
        std::sync::Arc::new(crate::timer::GlobalTimerRequests::new()),
        std::sync::Arc::new(privacy::start_suggestions_off()),
        std::sync::Arc::new(privacy::tips_off()),
        std::sync::Arc::new(privacy::lockscreen_ads_off()),
        std::sync::Arc::new(privacy::explorer_ads_off()),
        std::sync::Arc::new(privacy::ad_id_off()),
    ]
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::sys::Hive;

    pub const PATH: &str = r"Software\ApexTest";

    pub fn meta(id: &'static str) -> TweakMeta {
        TweakMeta {
            id,
            name: "Test tweak",
            description: "For tests.",
            category: Category::Gaming,
            risk: Risk::Safe,
            restart: Restart::None,
            recommended: true,
        }
    }

    pub fn two_values() -> StaticTweak {
        StaticTweak {
            meta: meta("two_values"),
            atoms: || {
                vec![
                    Atom::reg_dword(Hive::CurrentUser, PATH, "A", 1),
                    Atom::reg_dword(Hive::CurrentUser, PATH, "B", 1),
                ]
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::sys::fake::FakeSystem;
    use crate::sys::{Hive, RegData};

    #[test]
    fn detect_reports_applied_partial_and_not_applied() {
        let sys = FakeSystem::new();
        let tweak = two_values();
        assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::NotApplied);
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(1));
        assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::Partial);
        sys.registry
            .seed(Hive::CurrentUser, PATH, "B", RegData::Dword(1));
        assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::Applied);
    }

    #[test]
    fn plan_lists_only_what_would_change() {
        let sys = FakeSystem::new();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "A", RegData::Dword(1));
        let plan = two_values().plan(&sys.ctx).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(plan[0].target.ends_with(r"ApexTest\B"));
    }
}
