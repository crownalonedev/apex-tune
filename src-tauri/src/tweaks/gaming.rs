//! Gaming tweaks.

use super::{Atom, Category, Refresh, Restart, Risk, StaticTweak, Tweak, TweakMeta};
use crate::sys::Hive::{CurrentUser, LocalMachine as HKLM};
use crate::sys::{Hive, RegData, SysCtx, SysResult};

const MM_PROFILE: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";

fn meta(
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
        category: Category::Gaming,
        risk,
        restart,
        recommended,
    }
}

fn tweak(meta: TweakMeta, atoms: fn() -> Vec<Atom>) -> StaticTweak {
    StaticTweak {
        meta,
        atoms,
        refresh: Refresh::Nothing,
    }
}

pub fn game_mode() -> StaticTweak {
    tweak(
        meta(
            "game_mode",
            "Game Mode",
            "Lets Windows prioritise the game you're playing and hold back Windows Update installs and \
             notifications while it runs. On by default in Windows 11; this makes sure it stays on.",
            Risk::Safe,
            Restart::None,
            true,
        ),
        || {
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
    )
}

pub fn game_dvr_off() -> StaticTweak {
    tweak(
        meta(
            "game_dvr_off",
            "Turn off background recording",
            "Stops Xbox Game Bar from constantly recording the last few minutes of gameplay, which costs GPU \
             encoder time and disk writes. Game Bar itself and manual captures keep working.",
            Risk::Safe,
            Restart::SignOut,
            true,
        ),
        || {
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
    )
}

pub fn game_bar_controller_off() -> StaticTweak {
    tweak(
        meta(
            "game_bar_controller_off",
            "Xbox button doesn't open Game Bar",
            "Pressing the Xbox button on a controller no longer pops Game Bar over your game.",
            Risk::Safe,
            Restart::None,
            false,
        ),
        || {
            vec![Atom::reg_dword(
                CurrentUser,
                r"Software\Microsoft\GameBar",
                "UseNexusForGameBarEnabled",
                0,
            )]
        },
    )
}

pub fn hags() -> StaticTweak {
    tweak(
        meta(
            "hags",
            "Hardware-accelerated GPU scheduling",
            "Lets the GPU schedule its own work. Required for DLSS Frame Generation. In most games the FPS \
             difference is within noise, so leave it alone unless you use Frame Generation or measure a gain.",
            Risk::Moderate,
            Restart::Reboot,
            false,
        ),
        || {
            vec![Atom::reg_dword(
                HKLM,
                r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers",
                "HwSchMode",
                2,
            )]
        },
    )
}

pub fn mmcss_responsiveness() -> StaticTweak {
    tweak(
        meta(
            "mmcss_responsiveness",
            "Less CPU reserved for background tasks",
            "Windows reserves 20% of CPU time for low-priority work while multimedia apps run. This lowers it to \
             10%, the documented minimum (the often-quoted 0 is treated as 10 anyway).",
            Risk::Moderate,
            Restart::Reboot,
            false,
        ),
        || {
            vec![Atom::reg_dword(
                HKLM,
                MM_PROFILE,
                "SystemResponsiveness",
                10,
            )]
        },
    )
}

pub fn mmcss_games_task() -> StaticTweak {
    tweak(
        meta(
            "mmcss_games_task",
            "Higher scheduling priority for games that ask for it",
            "Raises the GPU and CPU priority Windows gives to the \"Games\" multimedia task. Only affects games that \
             register with that task, which few do, so the effect is small and varies by game.",
            Risk::Moderate,
            Restart::Reboot,
            false,
        ),
        || {
            let key = format!(r"{MM_PROFILE}\Tasks\Games");
            vec![
                Atom::reg_dword(HKLM, &key, "GPU Priority", 8),
                Atom::reg_dword(HKLM, &key, "Priority", 6),
                Atom::reg_sz(HKLM, &key, "Scheduling Category", "High"),
                Atom::reg_sz(HKLM, &key, "SFIO Priority", "High"),
            ]
        },
    )
}

/// Windows 11's "Optimizations for windowed games": flip-model presentation
/// for DX10/11 borderless and windowed games. The setting shares one string
/// with other per-user GPU preferences, so it's merged, not overwritten.
pub struct WindowedGames {
    meta: TweakMeta,
}

const DX_KEY: &str = r"Software\Microsoft\DirectX\UserGpuPreferences";
const DX_VALUE: &str = "DirectXUserGlobalSettings";

impl WindowedGames {
    pub fn new() -> Self {
        Self {
            meta: meta(
                "windowed_games",
                "Optimizations for windowed games",
                "Switches DirectX 10/11 games running borderless or windowed (Valorant and Fortnite included) to \
                 the flip presentation model: lower input latency, and VRR and Auto HDR work in windowed mode. A \
                 few games show tearing; turn it off for those.",
                Risk::Safe,
                Restart::None,
                true,
            ),
        }
    }
}

impl Default for WindowedGames {
    fn default() -> Self {
        Self::new()
    }
}

/// Sets `SwapEffectUpgradeEnable=1` in a `key=value;` list, keeping the rest.
pub fn merge_swap_effect(current: &str) -> String {
    let mut parts: Vec<String> = current
        .split(';')
        .map(str::trim)
        .filter(|p| {
            !p.is_empty()
                && !p
                    .to_ascii_lowercase()
                    .starts_with("swapeffectupgradeenable=")
        })
        .map(str::to_owned)
        .collect();
    parts.push("SwapEffectUpgradeEnable=1".into());
    parts.iter().map(|p| format!("{p};")).collect()
}

impl Tweak for WindowedGames {
    fn meta(&self) -> &TweakMeta {
        &self.meta
    }

    fn atoms(&self, ctx: &SysCtx) -> SysResult<Vec<Atom>> {
        let current = match ctx.registry.get(Hive::CurrentUser, DX_KEY, DX_VALUE)? {
            Some(RegData::Sz(s)) => s,
            _ => String::new(),
        };
        let enabled = current
            .to_ascii_lowercase()
            .replace(' ', "")
            .contains("swapeffectupgradeenable=1;");
        let target = if enabled {
            current
        } else {
            merge_swap_effect(&current)
        };
        Ok(vec![Atom::reg_sz(CurrentUser, DX_KEY, DX_VALUE, &target)])
    }

    fn after_change(&self, ctx: &SysCtx) -> SysResult<()> {
        ctx.desktop.broadcast_settings_change()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::guard;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::{Engine, TweakState};

    #[test]
    fn gaming_tweaks_pass_the_guard() {
        let sys = FakeSystem::new();
        for tweak in [
            game_mode(),
            game_dvr_off(),
            game_bar_controller_off(),
            hags(),
            mmcss_responsiveness(),
            mmcss_games_task(),
        ] {
            for atom in tweak.atoms(&sys.ctx).unwrap() {
                guard::check_atom(&atom).unwrap();
            }
            assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::NotApplied);
        }
    }

    #[test]
    fn swap_effect_merges_with_other_preferences() {
        assert_eq!(merge_swap_effect(""), "SwapEffectUpgradeEnable=1;");
        assert_eq!(
            merge_swap_effect("VRROptimizeEnable=0;"),
            "VRROptimizeEnable=0;SwapEffectUpgradeEnable=1;"
        );
        assert_eq!(
            merge_swap_effect("SwapEffectUpgradeEnable=0;AutoHDREnable=1;"),
            "AutoHDREnable=1;SwapEffectUpgradeEnable=1;"
        );
    }

    #[test]
    fn windowed_games_keeps_other_settings_and_reverts_exactly() {
        let sys = FakeSystem::new();
        sys.registry.seed(
            Hive::CurrentUser,
            DX_KEY,
            DX_VALUE,
            RegData::Sz("VRROptimizeEnable=0;".into()),
        );
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        let tweak = WindowedGames::new();
        engine.apply(&tweak).unwrap();
        assert_eq!(
            sys.registry.value(Hive::CurrentUser, DX_KEY, DX_VALUE),
            Some(RegData::Sz(
                "VRROptimizeEnable=0;SwapEffectUpgradeEnable=1;".into()
            ))
        );
        assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::Applied);
        engine.revert(&tweak).unwrap();
        assert_eq!(
            sys.registry.value(Hive::CurrentUser, DX_KEY, DX_VALUE),
            Some(RegData::Sz("VRROptimizeEnable=0;".into()))
        );
    }
}
