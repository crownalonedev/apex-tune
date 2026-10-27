//! Mouse and keyboard.

use super::{Atom, Category, Refresh, Restart, Risk, StaticTweak, TweakMeta};
use crate::sys::Hive::CurrentUser;

pub fn mouse_accel_off() -> StaticTweak {
    StaticTweak {
        meta: TweakMeta {
            id: "mouse_accel_off",
            name: "Turn off Enhance Pointer Precision",
            description: "Removes mouse acceleration, so the same physical movement always turns your aim by the \
                          same amount. Applies immediately.",
            category: Category::Input,
            risk: Risk::Safe,
            restart: Restart::None,
            recommended: true,
        },
        atoms: || {
            let key = r"Control Panel\Mouse";
            vec![
                Atom::reg_sz(CurrentUser, key, "MouseSpeed", "0"),
                Atom::reg_sz(CurrentUser, key, "MouseThreshold1", "0"),
                Atom::reg_sz(CurrentUser, key, "MouseThreshold2", "0"),
            ]
        },
        refresh: Refresh::Mouse,
    }
}

pub fn sticky_keys_shortcut_off() -> StaticTweak {
    StaticTweak {
        meta: TweakMeta {
            id: "sticky_keys_shortcut_off",
            name: "No Sticky Keys pop-up from Shift",
            description: "Pressing Shift five times (or holding it) mid-game no longer opens the Sticky Keys, Filter \
                          Keys or Toggle Keys prompt and minimises your game. The features themselves still work \
                          from Settings.",
            category: Category::Input,
            risk: Risk::Safe,
            restart: Restart::None,
            recommended: true,
        },
        atoms: || {
            let base = r"Control Panel\Accessibility";
            vec![
                // Windows defaults are 510, 62 and 126; these are the same with only the shortcut bit cleared.
                Atom::reg_sz(CurrentUser, &format!(r"{base}\StickyKeys"), "Flags", "506"),
                Atom::reg_sz(CurrentUser, &format!(r"{base}\ToggleKeys"), "Flags", "58"),
                Atom::reg_sz(
                    CurrentUser,
                    &format!(r"{base}\Keyboard Response"),
                    "Flags",
                    "122",
                ),
            ]
        },
        refresh: Refresh::Accessibility,
    }
}
