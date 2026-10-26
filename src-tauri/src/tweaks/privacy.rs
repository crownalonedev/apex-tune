//! Consumer suggestions, ads and advertising tracking. All per-user settings
//! that Windows exposes in Settings; nothing here touches security.

use super::{Atom, Category, Restart, Risk, StaticTweak, TweakMeta};
use crate::sys::Hive::CurrentUser;

const CDM: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const EXPLORER_ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";

fn meta(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    restart: Restart,
) -> TweakMeta {
    TweakMeta {
        id,
        name,
        description,
        category: Category::Privacy,
        risk: Risk::Safe,
        restart,
        recommended: true,
    }
}

pub fn start_suggestions_off() -> StaticTweak {
    StaticTweak {
        meta: meta(
            "start_suggestions_off",
            "No Start menu suggestions",
            "Removes app promotions and recommendations from Start, and stops Windows from silently installing \
             suggested apps.",
            Restart::Explorer,
        ),
        atoms: || {
            vec![
                Atom::reg_dword(CurrentUser, CDM, "SystemPaneSuggestionsEnabled", 0),
                Atom::reg_dword(CurrentUser, CDM, "SubscribedContent-338388Enabled", 0),
                Atom::reg_dword(CurrentUser, CDM, "SilentInstalledAppsEnabled", 0),
                Atom::reg_dword(
                    CurrentUser,
                    EXPLORER_ADVANCED,
                    "Start_IrisRecommendations",
                    0,
                ),
                Atom::reg_dword(
                    CurrentUser,
                    EXPLORER_ADVANCED,
                    "Start_AccountNotifications",
                    0,
                ),
            ]
        },
    }
}

pub fn tips_off() -> StaticTweak {
    StaticTweak {
        meta: meta(
            "tips_off",
            "No tips and \"finish setup\" prompts",
            "Turns off Windows tips, the welcome experience after updates, suggested content in Settings and the \
             \"Let's finish setting up your device\" screen.",
            Restart::None,
        ),
        atoms: || {
            vec![
                Atom::reg_dword(CurrentUser, CDM, "SubscribedContent-338389Enabled", 0),
                Atom::reg_dword(CurrentUser, CDM, "SoftLandingEnabled", 0),
                Atom::reg_dword(CurrentUser, CDM, "SubscribedContent-310093Enabled", 0),
                Atom::reg_dword(CurrentUser, CDM, "SubscribedContent-338393Enabled", 0),
                Atom::reg_dword(CurrentUser, CDM, "SubscribedContent-353694Enabled", 0),
                Atom::reg_dword(CurrentUser, CDM, "SubscribedContent-353696Enabled", 0),
                Atom::reg_dword(
                    CurrentUser,
                    r"Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement",
                    "ScoobeSystemSettingEnabled",
                    0,
                ),
            ]
        },
    }
}

pub fn lockscreen_ads_off() -> StaticTweak {
    StaticTweak {
        meta: meta(
            "lockscreen_ads_off",
            "No lock screen ads",
            "Removes the fun facts and tips layered on the lock screen. Windows Spotlight pictures are left alone.",
            Restart::None,
        ),
        atoms: || {
            vec![
                Atom::reg_dword(CurrentUser, CDM, "RotatingLockScreenOverlayEnabled", 0),
                Atom::reg_dword(CurrentUser, CDM, "SubscribedContent-338387Enabled", 0),
            ]
        },
    }
}

pub fn explorer_ads_off() -> StaticTweak {
    StaticTweak {
        meta: meta(
            "explorer_ads_off",
            "No File Explorer promotions",
            "Stops sync-provider notifications (OneDrive and Microsoft 365 promotions) in File Explorer.",
            Restart::Explorer,
        ),
        atoms: || {
            vec![Atom::reg_dword(
                CurrentUser,
                EXPLORER_ADVANCED,
                "ShowSyncProviderNotifications",
                0,
            )]
        },
    }
}

pub fn ad_id_off() -> StaticTweak {
    StaticTweak {
        meta: meta(
            "ad_id_off",
            "No advertising ID",
            "Turns off the advertising ID apps use for personalised ads, and tailored experiences based on your \
             diagnostic data.",
            Restart::None,
        ),
        atoms: || {
            vec![
                Atom::reg_dword(
                    CurrentUser,
                    r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
                    "Enabled",
                    0,
                ),
                Atom::reg_dword(
                    CurrentUser,
                    r"Software\Microsoft\Windows\CurrentVersion\Privacy",
                    "TailoredExperiencesWithDiagnosticDataEnabled",
                    0,
                ),
            ]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::Tweak;

    #[test]
    fn privacy_tweaks_pass_the_guard() {
        let sys = FakeSystem::new();
        for tweak in [
            start_suggestions_off(),
            tips_off(),
            lockscreen_ads_off(),
            explorer_ads_off(),
            ad_id_off(),
        ] {
            for atom in tweak.atoms(&sys.ctx).unwrap() {
                guard::check_atom(&atom).unwrap();
            }
        }
    }
}
