//! Consumer suggestions, ads and advertising tracking. All per-user settings
//! that Windows exposes in Settings; nothing here touches security.

use super::{Atom, Category, Restart, Risk, StaticTweak, TweakMeta};
use crate::sys::Hive::{CurrentUser, LocalMachine as HKLM};

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
        refresh: crate::tweaks::Refresh::Nothing,
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
        refresh: crate::tweaks::Refresh::Nothing,
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
        refresh: crate::tweaks::Refresh::Nothing,
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
        refresh: crate::tweaks::Refresh::Nothing,
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
        refresh: crate::tweaks::Refresh::Nothing,
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

fn custom(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    risk: Risk,
    restart: Restart,
    recommended: bool,
) -> TweakMeta {
    TweakMeta {
        id,
        name,
        description,
        category: Category::Privacy,
        risk,
        restart,
        recommended,
    }
}

pub fn widgets_off() -> StaticTweak {
    StaticTweak {
        refresh: crate::tweaks::Refresh::Nothing,
        meta: custom(
            "widgets_off",
            "Turn off Widgets",
            "Removes the Widgets board and its always-running Widgets and WebView2 processes, which use a few \
             hundred MB of RAM. Uses Microsoft's \"Allow widgets\" policy.",
            Risk::Safe,
            Restart::Explorer,
            true,
        ),
        atoms: || {
            vec![Atom::reg_dword(
                HKLM,
                r"SOFTWARE\Policies\Microsoft\Dsh",
                "AllowNewsAndInterests",
                0,
            )]
        },
    }
}

pub fn edge_background_off() -> StaticTweak {
    StaticTweak {
        refresh: crate::tweaks::Refresh::Nothing,
        meta: custom(
            "edge_background_off",
            "Stop Edge running in the background",
            "Turns off Edge Startup Boost and background mode, so no Edge processes run when Edge is closed. Edge \
             opens a moment slower and shows \"managed by your organization\" in its settings, because these are \
             Edge policies.",
            Risk::Safe,
            Restart::None,
            true,
        ),
        atoms: || {
            vec![
                Atom::reg_dword(
                    HKLM,
                    r"SOFTWARE\Policies\Microsoft\Edge",
                    "StartupBoostEnabled",
                    0,
                ),
                Atom::reg_dword(
                    HKLM,
                    r"SOFTWARE\Policies\Microsoft\Edge",
                    "BackgroundModeEnabled",
                    0,
                ),
            ]
        },
    }
}

pub fn background_apps_off() -> StaticTweak {
    StaticTweak {
        refresh: crate::tweaks::Refresh::Nothing,
        meta: custom(
            "background_apps_off",
            "Stop Store apps running in the background",
            "Store apps (Mail, Photos, Phone Link and others) can no longer run or sync while closed. Breaks Windows \
             Spotlight wallpapers and live notifications from those apps.",
            Risk::Moderate,
            Restart::SignOut,
            false,
        ),
        atoms: || {
            vec![Atom::reg_dword(
                CurrentUser,
                r"Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications",
                "GlobalUserDisabled",
                1,
            )]
        },
    }
}

pub fn web_search_off() -> StaticTweak {
    StaticTweak {
        refresh: crate::tweaks::Refresh::Nothing,
        meta: custom(
            "web_search_off",
            "No web results in Start search",
            "Start menu search only searches your PC, without Bing results, so it's faster and keeps typed searches \
             local.",
            Risk::Moderate,
            Restart::Explorer,
            false,
        ),
        atoms: || {
            vec![Atom::reg_dword(
                CurrentUser,
                r"Software\Policies\Microsoft\Windows\Explorer",
                "DisableSearchBoxSuggestions",
                1,
            )]
        },
    }
}

pub fn telemetry_required() -> StaticTweak {
    StaticTweak {
        refresh: crate::tweaks::Refresh::Nothing,
        meta: custom(
            "telemetry_required",
            "Send only required diagnostic data",
            "Sets diagnostic data to Required, the lowest level Windows Home and Pro honour. Updates and security \
             are unaffected.",
            Risk::Safe,
            Restart::None,
            false,
        ),
        atoms: || {
            vec![Atom::reg_dword(
                HKLM,
                r"SOFTWARE\Policies\Microsoft\Windows\DataCollection",
                "AllowTelemetry",
                1,
            )]
        },
    }
}

pub fn recall_off() -> StaticTweak {
    StaticTweak {
        refresh: crate::tweaks::Refresh::Nothing,
        meta: custom(
            "recall_off",
            "Turn off Recall snapshots",
            "Stops Windows Recall from saving screenshots of your activity on Copilot+ PCs. Has no effect on other PCs.",
            Risk::Safe,
            Restart::SignOut,
            false,
        ),
        atoms: || {
            vec![Atom::reg_dword(
                CurrentUser,
                r"Software\Policies\Microsoft\Windows\WindowsAI",
                "DisableAIDataAnalysis",
                1,
            )]
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
            widgets_off(),
            edge_background_off(),
            background_apps_off(),
            web_search_off(),
            telemetry_required(),
            recall_off(),
        ] {
            for atom in tweak.atoms(&sys.ctx).unwrap() {
                guard::check_atom(&atom).unwrap();
            }
        }
    }
}
