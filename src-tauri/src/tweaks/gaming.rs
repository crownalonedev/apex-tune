//! Gaming tweaks. More arrive in the Custom Tweaks stage.

use super::{Atom, Category, Restart, Risk, StaticTweak, TweakMeta};
use crate::sys::Hive::CurrentUser;

pub fn game_mode() -> StaticTweak {
    StaticTweak {
        meta: TweakMeta {
            id: "game_mode",
            name: "Game Mode",
            description: "Lets Windows prioritise the game you're playing and hold back Windows Update installs and \
                          notifications while it runs. On by default in Windows 11; this makes sure it stays on.",
            category: Category::Gaming,
            risk: Risk::Safe,
            restart: Restart::None,
            recommended: true,
        },
        atoms: || {
            vec![
                Atom::reg_dword(
                    CurrentUser,
                    r"Software\Microsoft\GameBar",
                    "AutoGameModeEnabled",
                    1,
                ),
                Atom::reg_dword(
                    CurrentUser,
                    r"Software\Microsoft\GameBar",
                    "AllowAutoGameMode",
                    1,
                ),
            ]
        },
    }
}

pub fn game_dvr_off() -> StaticTweak {
    StaticTweak {
        meta: TweakMeta {
            id: "game_dvr_off",
            name: "Turn off background recording",
            description: "Stops Xbox Game Bar from constantly recording the last few minutes of gameplay, which costs \
                          GPU encoder time and disk writes. Game Bar itself and manual captures keep working.",
            category: Category::Gaming,
            risk: Risk::Safe,
            restart: Restart::SignOut,
            recommended: true,
        },
        atoms: || {
            vec![
                Atom::reg_dword(CurrentUser, r"System\GameConfigStore", "GameDVR_Enabled", 0),
                Atom::reg_dword(
                    CurrentUser,
                    r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
                    "AppCaptureEnabled",
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
    use crate::tweaks::{Tweak, TweakState};

    #[test]
    fn gaming_tweaks_pass_the_guard_and_detect() {
        let sys = FakeSystem::new();
        for tweak in [game_mode(), game_dvr_off()] {
            for atom in tweak.atoms(&sys.ctx).unwrap() {
                guard::check_atom(&atom).unwrap();
            }
            assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::NotApplied);
        }
    }
}
