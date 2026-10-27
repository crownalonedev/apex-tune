//! Desktop visual effects. These make Windows feel snappier; they don't
//! change FPS in fullscreen games, and the descriptions say so.

use super::{Atom, Category, Refresh, Restart, Risk, StaticTweak, TweakMeta};
use crate::sys::Hive::CurrentUser;
use crate::sys::SpiFlag;

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
        category: Category::Visual,
        risk: Risk::Safe,
        restart,
        recommended: false,
    }
}

const EXPLORER_ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";

pub fn transparency_off() -> StaticTweak {
    StaticTweak {
        meta: meta(
            "transparency_off",
            "Turn off transparency",
            "Solid taskbar, Start and window backgrounds. Slightly less work for the GPU on the desktop.",
            Restart::None,
        ),
        atoms: || {
            vec![Atom::reg_dword(
                CurrentUser,
                r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "EnableTransparency",
                0,
            )]
        },
        refresh: Refresh::Broadcast,
    }
}

pub fn animations_off() -> StaticTweak {
    StaticTweak {
        meta: meta(
            "animations_off",
            "Turn off window animations",
            "Windows open, close, minimise and maximise instantly instead of animating.",
            Restart::Explorer,
        ),
        atoms: || {
            vec![
                Atom::Spi {
                    flag: SpiFlag::ClientAreaAnimation,
                    value: false,
                },
                Atom::Spi {
                    flag: SpiFlag::MinimizeAnimation,
                    value: false,
                },
                Atom::reg_dword(CurrentUser, EXPLORER_ADVANCED, "TaskbarAnimations", 0),
            ]
        },
        refresh: Refresh::Nothing,
    }
}

pub fn best_performance_visuals() -> StaticTweak {
    StaticTweak {
        meta: meta(
            "best_performance_visuals",
            "Adjust for best performance (keep smooth fonts)",
            "The \"Adjust for best performance\" visual effects preset, except that text stays smooth. Menus, \
             tooltips, shadows and list effects are turned off; the desktop feels more immediate.",
            Restart::SignOut,
        ),
        atoms: || {
            let off = |flag| Atom::Spi { flag, value: false };
            vec![
                off(SpiFlag::MenuAnimation),
                off(SpiFlag::ComboBoxAnimation),
                off(SpiFlag::ListBoxSmoothScrolling),
                off(SpiFlag::TooltipAnimation),
                off(SpiFlag::SelectionFade),
                off(SpiFlag::DropShadow),
                off(SpiFlag::CursorShadow),
                off(SpiFlag::DragFullWindows),
                Atom::Spi {
                    flag: SpiFlag::FontSmoothing,
                    value: true,
                },
                Atom::reg_dword(CurrentUser, EXPLORER_ADVANCED, "ListviewAlphaSelect", 0),
                Atom::reg_dword(CurrentUser, EXPLORER_ADVANCED, "ListviewShadow", 0),
                Atom::reg_dword(
                    CurrentUser,
                    r"Software\Microsoft\Windows\DWM",
                    "EnableAeroPeek",
                    0,
                ),
                // 3 = "Custom", which is what Windows shows once individual effects differ from a preset.
                Atom::reg_dword(
                    CurrentUser,
                    r"Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects",
                    "VisualFXSetting",
                    3,
                ),
            ]
        },
        refresh: Refresh::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::{Engine, Tweak, TweakState};

    #[test]
    fn visual_effects_round_trip_through_spi() {
        let sys = FakeSystem::new();
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        let tweak = best_performance_visuals();
        engine.apply(&tweak).unwrap();
        assert!(!sys.ctx.desktop.spi_get(SpiFlag::MenuAnimation).unwrap());
        assert!(sys.ctx.desktop.spi_get(SpiFlag::FontSmoothing).unwrap());
        assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::Applied);
        engine.revert(&tweak).unwrap();
        assert!(sys.ctx.desktop.spi_get(SpiFlag::MenuAnimation).unwrap());
    }
}
