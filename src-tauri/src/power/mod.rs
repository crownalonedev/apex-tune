//! The Apex Tune Gaming power plan and plan switching.
//!
//! The plan has a fixed GUID, so Apex Tune always knows which plan is its own
//! and only ever deletes that one.

use serde::Serialize;

use crate::sys::{PowerSource, SysCtx, SysResult};
use crate::tweaks::atom::{Atom, PowerUnit};
use crate::tweaks::{Category, Restart, Risk, Tweak, TweakMeta};

pub const APEX_PLAN: &str = "c855ae8b-fff9-45c7-8c29-99f6f893b54a";
pub const ULTIMATE_PERFORMANCE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
pub const HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
pub const BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub const POWER_SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";

const BUILT_IN: &[&str] = &[
    BALANCED,
    HIGH_PERFORMANCE,
    POWER_SAVER,
    ULTIMATE_PERFORMANCE,
];

const SUB_PROCESSOR: &str = "54533251-82be-4824-96c1-47b60b740d00";
const SUB_USB: &str = "2a737441-1930-4402-8d77-b2bebba308a3";
const SUB_PCIE: &str = "501a4d13-42af-4429-9fd1-a8218c268e20";
const SUB_DISK: &str = "0012ee47-9041-4b5d-9b77-535fba8b1442";
const SUB_VIDEO: &str = "7516b95f-f776-4464-8c53-06167f40cc99";
const SUB_SLEEP: &str = "238c9fa8-0aad-41ed-83f4-97be242c8f20";

#[derive(Debug, Clone, Copy)]
enum Unit {
    Percent,
    Seconds,
    OnOff,
    Aspm,
}

impl Unit {
    fn power_unit(self) -> PowerUnit {
        match self {
            Unit::Percent => PowerUnit::Percent,
            Unit::Seconds => PowerUnit::Seconds,
            Unit::OnOff => PowerUnit::Choice(vec!["Off".into(), "On".into()]),
            Unit::Aspm => PowerUnit::Choice(vec![
                "Off".into(),
                "Moderate savings".into(),
                "Maximum savings".into(),
            ]),
        }
    }
}

/// One tuned setting. DC values are only written by the laptop variant.
struct PlanSetting {
    label: &'static str,
    subgroup: &'static str,
    setting: &'static str,
    unit: Unit,
    /// Plugged in, both variants.
    ac: u32,
    /// Laptop variant, plugged in, when it differs from `ac`.
    laptop_ac: Option<u32>,
    /// Laptop variant on battery. `None` leaves the source plan's value.
    laptop_dc: Option<u32>,
}

const SETTINGS: &[PlanSetting] = &[
    PlanSetting {
        label: "Minimum processor state",
        subgroup: SUB_PROCESSOR,
        setting: "893dee8e-2bef-41e0-89c6-b55d0929964c",
        unit: Unit::Percent,
        ac: 100,
        laptop_ac: None,
        laptop_dc: Some(5),
    },
    PlanSetting {
        label: "Core parking (minimum unparked cores)",
        subgroup: SUB_PROCESSOR,
        setting: "0cc5b647-c1df-4637-891a-dec35c318583",
        unit: Unit::Percent,
        ac: 100,
        laptop_ac: None,
        laptop_dc: None,
    },
    PlanSetting {
        label: "USB selective suspend",
        subgroup: SUB_USB,
        setting: "48e6b7a6-50f5-4782-a5d4-53bb8f07e226",
        unit: Unit::OnOff,
        ac: 0,
        laptop_ac: None,
        laptop_dc: Some(1),
    },
    PlanSetting {
        label: "PCI Express Link State Power Management",
        subgroup: SUB_PCIE,
        setting: "ee12f906-d277-404b-b6da-e5fa1a576df5",
        unit: Unit::Aspm,
        ac: 0,
        laptop_ac: None,
        laptop_dc: Some(1),
    },
    PlanSetting {
        label: "Turn off hard disk after",
        subgroup: SUB_DISK,
        setting: "6738e2c4-e8a5-4a42-b16a-e040e769756e",
        unit: Unit::Seconds,
        ac: 0,
        laptop_ac: None,
        laptop_dc: Some(600),
    },
    PlanSetting {
        label: "Turn off display after",
        subgroup: SUB_VIDEO,
        setting: "3c0bc021-c8a8-4e07-a973-6b14cbcb2b7e",
        unit: Unit::Seconds,
        ac: 900,
        laptop_ac: None,
        laptop_dc: Some(300),
    },
    PlanSetting {
        label: "Sleep after",
        subgroup: SUB_SLEEP,
        setting: "29f6c1db-86da-48c5-9fdb-f2b67b1f44da",
        unit: Unit::Seconds,
        ac: 0,
        laptop_ac: Some(3600),
        laptop_dc: Some(900),
    },
];

fn setting_atom(s: &PlanSetting, source: PowerSource, value: u32) -> Atom {
    Atom::PowerSetting {
        scheme: APEX_PLAN.into(),
        subgroup: s.subgroup.into(),
        setting: s.setting.into(),
        label: s.label.into(),
        unit: s.unit.power_unit(),
        source,
        value,
    }
}

/// The atoms for one variant: create the plan, tune it, make it active.
/// Reverting runs them backwards: previous plan active again, then the plan deleted.
pub fn plan_atoms(laptop: bool) -> Vec<Atom> {
    let mut atoms = vec![Atom::PowerPlan {
        guid: APEX_PLAN.into(),
        // Ultimate Performance can be missing on laptops and Modern Standby PCs.
        sources: vec![
            ULTIMATE_PERFORMANCE.into(),
            HIGH_PERFORMANCE.into(),
            BALANCED.into(),
        ],
        name: if laptop {
            "Apex Tune Gaming (laptop)"
        } else {
            "Apex Tune Gaming"
        }
        .into(),
        description:
            "Created by Apex Tune for low-latency gaming. Revert it in Apex Tune to remove it."
                .into(),
    }];
    for s in SETTINGS {
        let ac = if laptop {
            s.laptop_ac.unwrap_or(s.ac)
        } else {
            s.ac
        };
        atoms.push(setting_atom(s, PowerSource::Ac, ac));
        if laptop && let Some(dc) = s.laptop_dc {
            atoms.push(setting_atom(s, PowerSource::Dc, dc));
        }
    }
    atoms.push(Atom::ActivePowerPlan {
        guid: APEX_PLAN.into(),
    });
    atoms
}

pub struct GamingPlan {
    meta: TweakMeta,
}

impl GamingPlan {
    pub fn new() -> Self {
        Self {
            meta: TweakMeta {
                id: "power_plan",
                name: "Apex Tune Gaming power plan",
                description: "A copy of Ultimate Performance with the CPU held at full speed, core parking off, and USB \
                              and PCIe power saving off while plugged in, which smooths out frame-time spikes. Idle \
                              power use and temperatures go up. On laptops, battery settings stay power-saving.",
                category: Category::Power,
                risk: Risk::Safe,
                restart: Restart::None,
                recommended: true,
            },
        }
    }
}

impl Default for GamingPlan {
    fn default() -> Self {
        Self::new()
    }
}

impl Tweak for GamingPlan {
    fn meta(&self) -> &TweakMeta {
        &self.meta
    }

    fn atoms(&self, ctx: &SysCtx) -> SysResult<Vec<Atom>> {
        Ok(plan_atoms(ctx.power.has_battery()?))
    }
}

// ---- Overview for the Power page ---------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInfo {
    pub guid: String,
    pub name: String,
    pub active: bool,
    pub built_in: bool,
    pub apex: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingRow {
    pub label: String,
    pub target_ac: String,
    pub target_dc: Option<String>,
    /// Current values in the Apex plan, if it's installed.
    pub current_ac: Option<String>,
    pub current_dc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerOverview {
    pub plans: Vec<PlanInfo>,
    pub laptop: bool,
    pub balanced_installed: bool,
    pub settings: Vec<SettingRow>,
}

pub fn overview(ctx: &SysCtx) -> SysResult<PowerOverview> {
    let laptop = ctx.power.has_battery()?;
    let active = ctx.power.active()?;
    let plans: Vec<PlanInfo> = ctx
        .power
        .schemes()?
        .into_iter()
        .map(|s| PlanInfo {
            active: s.guid.eq_ignore_ascii_case(&active),
            built_in: BUILT_IN.iter().any(|b| b.eq_ignore_ascii_case(&s.guid)),
            apex: s.guid.eq_ignore_ascii_case(APEX_PLAN),
            guid: s.guid,
            name: s.name,
        })
        .collect();
    let installed = plans.iter().any(|p| p.apex);

    let read = |s: &PlanSetting, source| -> SysResult<Option<String>> {
        if !installed {
            return Ok(None);
        }
        Ok(ctx
            .power
            .read(APEX_PLAN, s.subgroup, s.setting, source)?
            .map(|v| s.unit.power_unit().format(v)))
    };
    let mut settings = Vec::new();
    for s in SETTINGS {
        let unit = s.unit.power_unit();
        let ac = if laptop {
            s.laptop_ac.unwrap_or(s.ac)
        } else {
            s.ac
        };
        settings.push(SettingRow {
            label: s.label.into(),
            target_ac: unit.format(ac),
            target_dc: laptop
                .then_some(s.laptop_dc)
                .flatten()
                .map(|v| unit.format(v)),
            current_ac: read(s, PowerSource::Ac)?,
            current_dc: if laptop {
                read(s, PowerSource::Dc)?
            } else {
                None
            },
        });
    }

    Ok(PowerOverview {
        balanced_installed: plans.iter().any(|p| p.guid.eq_ignore_ascii_case(BALANCED)),
        plans,
        laptop,
        settings,
    })
}

/// The atoms for switching to another installed plan.
pub fn switch_atoms(guid: &str) -> Vec<Atom> {
    vec![Atom::ActivePowerPlan {
        guid: guid.to_ascii_lowercase(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::{BALANCED as FAKE_BALANCED, FakeSystem};
    use crate::tweaks::{Engine, TweakState};
    use std::sync::Arc;

    fn engine(sys: &FakeSystem, dir: &std::path::Path) -> Engine {
        Engine::new(
            sys.ctx.clone(),
            Journal::open(dir).unwrap(),
            vec![Arc::new(GamingPlan::new())],
        )
    }

    #[test]
    fn installs_from_ultimate_performance_and_reverts_completely() {
        let sys = FakeSystem::new();
        sys.power.add_template(ULTIMATE_PERFORMANCE);
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&sys, dir.path());
        let plan = GamingPlan::new();

        engine.apply(&plan).unwrap();
        assert_eq!(sys.ctx.power.active().unwrap(), APEX_PLAN);
        let name = sys
            .ctx
            .power
            .schemes()
            .unwrap()
            .into_iter()
            .find(|s| s.guid == APEX_PLAN)
            .unwrap()
            .name;
        assert_eq!(name, "Apex Tune Gaming");
        assert_eq!(
            sys.power.value(
                APEX_PLAN,
                SUB_PROCESSOR,
                SETTINGS[0].setting,
                PowerSource::Ac
            ),
            Some(100)
        );
        assert_eq!(
            sys.power.value(
                APEX_PLAN,
                SUB_PROCESSOR,
                SETTINGS[0].setting,
                PowerSource::Dc
            ),
            None,
            "desktop leaves DC alone"
        );
        assert_eq!(plan.detect(&sys.ctx).unwrap(), TweakState::Applied);

        engine.revert(&plan).unwrap();
        assert_eq!(sys.ctx.power.active().unwrap(), FAKE_BALANCED);
        assert!(!sys.ctx.power.exists(APEX_PLAN).unwrap());
    }

    #[test]
    fn falls_back_when_ultimate_performance_is_missing() {
        let sys = FakeSystem::new(); // only Balanced, as on Modern Standby laptops
        let dir = tempfile::tempdir().unwrap();
        engine(&sys, dir.path()).apply(&GamingPlan::new()).unwrap();
        assert!(sys.ctx.power.exists(APEX_PLAN).unwrap());
    }

    #[test]
    fn laptop_variant_keeps_battery_settings_power_saving() {
        let sys = FakeSystem::new();
        *sys.power.battery.lock().unwrap() = true;
        let dir = tempfile::tempdir().unwrap();
        engine(&sys, dir.path()).apply(&GamingPlan::new()).unwrap();
        let min_state = SETTINGS[0].setting;
        assert_eq!(
            sys.power
                .value(APEX_PLAN, SUB_PROCESSOR, min_state, PowerSource::Ac),
            Some(100)
        );
        assert_eq!(
            sys.power
                .value(APEX_PLAN, SUB_PROCESSOR, min_state, PowerSource::Dc),
            Some(5)
        );
        let sleep = SETTINGS[6].setting;
        assert_eq!(
            sys.power
                .value(APEX_PLAN, SUB_SLEEP, sleep, PowerSource::Ac),
            Some(3600)
        );
    }

    #[test]
    fn failed_setting_removes_the_half_made_plan() {
        let sys = FakeSystem::new();
        sys.power
            .fail_settings
            .lock()
            .unwrap()
            .push(SETTINGS[3].setting.into());
        let dir = tempfile::tempdir().unwrap();
        let err = engine(&sys, dir.path())
            .apply(&GamingPlan::new())
            .unwrap_err();
        assert_eq!(err.kind(), "rolledBack");
        assert!(!sys.ctx.power.exists(APEX_PLAN).unwrap());
        assert_eq!(sys.ctx.power.active().unwrap(), FAKE_BALANCED);
    }

    #[test]
    fn switching_plans_is_journaled_and_revertible() {
        let sys = FakeSystem::new();
        sys.power
            .schemes
            .lock()
            .unwrap()
            .push(crate::sys::PowerScheme {
                guid: HIGH_PERFORMANCE.into(),
                name: "High performance".into(),
            });
        let dir = tempfile::tempdir().unwrap();
        let mut engine = engine(&sys, dir.path());
        engine
            .apply_action(
                "power.switch",
                "Switched power plan",
                switch_atoms(HIGH_PERFORMANCE),
            )
            .unwrap();
        assert_eq!(sys.ctx.power.active().unwrap(), HIGH_PERFORMANCE);
        let report = engine.revert_everything();
        assert!(report.failed.is_empty());
        assert_eq!(sys.ctx.power.active().unwrap(), FAKE_BALANCED);
    }

    #[test]
    fn overview_marks_plans_and_formats_values() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        engine(&sys, dir.path()).apply(&GamingPlan::new()).unwrap();
        let o = overview(&sys.ctx).unwrap();
        assert!(o.plans.iter().any(|p| p.apex && p.active));
        assert!(o.plans.iter().any(|p| p.built_in && !p.active));
        assert_eq!(o.settings[4].target_ac, "Never");
        assert_eq!(o.settings[5].current_ac.as_deref(), Some("15 min"));
        assert_eq!(o.settings[3].target_ac, "Off");
    }
}
