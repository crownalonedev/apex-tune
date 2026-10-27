//! Windows Update behaviour and startup.

use super::{Applicability, Atom, Category, Refresh, Restart, Risk, StaticTweak, Tweak, TweakMeta};
use crate::hw::windows_version;
use crate::sys::Hive::LocalMachine as HKLM;
use crate::sys::{SysCtx, SysResult};

const WU_POLICY: &str = r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate";

pub fn drivers_from_windows_update_off() -> StaticTweak {
    StaticTweak {
        meta: TweakMeta {
            id: "wu_drivers_off",
            name: "Windows Update leaves drivers alone",
            description: "Stops Windows Update from installing driver updates, so it can't replace the NVIDIA \
                          driver you chose with an older one. Security and quality updates still install. Update \
                          drivers yourself from the vendor.",
            category: Category::Updates,
            risk: Risk::Moderate,
            restart: Restart::None,
            recommended: false,
        },
        atoms: || {
            vec![Atom::reg_dword(
                HKLM,
                WU_POLICY,
                "ExcludeWUDriversInQualityUpdate",
                1,
            )]
        },
        refresh: Refresh::Nothing,
    }
}

pub fn delivery_optimization_local() -> StaticTweak {
    StaticTweak {
        meta: TweakMeta {
            id: "delivery_optimization_local",
            name: "Don't upload updates to other PCs",
            description: "Windows downloads updates straight from Microsoft and stops sharing pieces of them with \
                          other PCs, so your upload bandwidth isn't used while you play.",
            category: Category::Updates,
            risk: Risk::Safe,
            restart: Restart::None,
            recommended: false,
        },
        atoms: || {
            vec![Atom::reg_dword(
                HKLM,
                r"SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization",
                "DODownloadMode",
                0,
            )]
        },
        refresh: Refresh::Nothing,
    }
}

/// Feature-update deferral. A Group Policy setting, so Home ignores it.
pub struct DeferFeatureUpdates {
    meta: TweakMeta,
}

impl DeferFeatureUpdates {
    pub fn new() -> Self {
        Self {
            meta: TweakMeta {
                id: "defer_feature_updates",
                name: "Delay yearly feature updates by 180 days",
                description: "Big Windows version upgrades arrive about six months later, after early bugs are fixed. \
                              Monthly security updates are not delayed.",
                category: Category::Updates,
                risk: Risk::Moderate,
                restart: Restart::None,
                recommended: false,
            },
        }
    }
}

impl Default for DeferFeatureUpdates {
    fn default() -> Self {
        Self::new()
    }
}

impl Tweak for DeferFeatureUpdates {
    fn meta(&self) -> &TweakMeta {
        &self.meta
    }

    fn applicable(&self, ctx: &SysCtx) -> SysResult<Applicability> {
        Ok(match windows_version::read(ctx)? {
            Some(v) if v.is_home() => {
                Applicability::NotApplicable("Windows Home ignores this policy.".into())
            }
            _ => Applicability::Applicable,
        })
    }

    fn atoms(&self, _ctx: &SysCtx) -> SysResult<Vec<Atom>> {
        Ok(vec![
            Atom::reg_dword(HKLM, WU_POLICY, "DeferFeatureUpdates", 1),
            Atom::reg_dword(HKLM, WU_POLICY, "DeferFeatureUpdatesPeriodInDays", 180),
        ])
    }
}

pub fn fast_startup_off() -> StaticTweak {
    StaticTweak {
        meta: TweakMeta {
            id: "fast_startup_off",
            name: "Turn off Fast Startup",
            description: "Shutting down then really shuts down instead of hibernating the kernel, so drivers start \
                          fresh after a driver update and uptime resets. Cold boots take a few seconds longer.",
            category: Category::System,
            risk: Risk::Moderate,
            restart: Restart::Reboot,
            recommended: false,
        },
        atoms: || {
            vec![Atom::reg_dword(
                HKLM,
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Power",
                "HiberbootEnabled",
                0,
            )]
        },
        refresh: Refresh::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::fake::FakeSystem;
    use crate::sys::{Hive, RegData};
    use crate::tweaks::TweakState;

    #[test]
    fn update_tweaks_pass_the_guard() {
        let sys = FakeSystem::new();
        for t in [
            drivers_from_windows_update_off(),
            delivery_optimization_local(),
            fast_startup_off(),
        ] {
            for a in t.atoms(&sys.ctx).unwrap() {
                crate::guard::check_atom(&a).unwrap();
            }
        }
        for a in DeferFeatureUpdates::new().atoms(&sys.ctx).unwrap() {
            crate::guard::check_atom(&a).unwrap();
        }
    }

    #[test]
    fn deferral_not_applicable_on_home() {
        let sys = FakeSystem::new();
        let cv = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        sys.registry.seed(
            Hive::LocalMachine,
            cv,
            "CurrentBuildNumber",
            RegData::Sz("26100".into()),
        );
        sys.registry.seed(
            Hive::LocalMachine,
            cv,
            "EditionID",
            RegData::Sz("Core".into()),
        );
        assert!(matches!(
            DeferFeatureUpdates::new().detect(&sys.ctx).unwrap(),
            TweakState::NotApplicable(_)
        ));
    }
}
