//! SystemParametersInfo, display modes and settings broadcasts.

use std::ffi::c_void;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DEVMODEW, DISP_CHANGE_SUCCESSFUL,
    DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICEW, DM_DISPLAYFREQUENCY,
    ENUM_CURRENT_SETTINGS, ENUM_DISPLAY_SETTINGS_MODE, EnumDisplayDevicesW, EnumDisplaySettingsW,
};
use windows::Win32::UI::Accessibility::{FILTERKEYS, STICKYKEYS, TOGGLEKEYS};
use windows::Win32::UI::WindowsAndMessaging::{
    ANIMATIONINFO, HWND_BROADCAST, SMTO_ABORTIFHUNG, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
    SYSTEM_PARAMETERS_INFO_ACTION, SendMessageTimeoutW, SystemParametersInfoW, WM_SETTINGCHANGE,
};
use windows::core::{HSTRING, PCWSTR};

use super::sys_err;
use crate::sys::{
    Desktop, DisplayInfo, Hive, RegData, Registry, SpiFlag, SysError, SysErrorKind, SysResult,
};

const SPI_SETMOUSE: u32 = 0x0004;
const SPI_GETANIMATION: u32 = 0x0048;
const SPI_SETANIMATION: u32 = 0x0049;
const SPI_GETFILTERKEYS: u32 = 0x0032;
const SPI_SETFILTERKEYS: u32 = 0x0033;
const SPI_GETTOGGLEKEYS: u32 = 0x0034;
const SPI_SETTOGGLEKEYS: u32 = 0x0035;
const SPI_GETSTICKYKEYS: u32 = 0x003A;
const SPI_SETSTICKYKEYS: u32 = 0x003B;

/// (get, set, value passed in uiParam instead of pvParam when setting)
fn codes(flag: SpiFlag) -> (u32, u32, bool) {
    match flag {
        SpiFlag::MenuAnimation => (0x1002, 0x1003, false),
        SpiFlag::ComboBoxAnimation => (0x1004, 0x1005, false),
        SpiFlag::ListBoxSmoothScrolling => (0x1006, 0x1007, false),
        SpiFlag::SelectionFade => (0x1014, 0x1015, false),
        SpiFlag::TooltipAnimation => (0x1016, 0x1017, false),
        SpiFlag::CursorShadow => (0x101A, 0x101B, false),
        SpiFlag::DropShadow => (0x1024, 0x1025, false),
        SpiFlag::ClientAreaAnimation => (0x1042, 0x1043, false),
        SpiFlag::DragFullWindows => (0x0026, 0x0025, true),
        SpiFlag::FontSmoothing => (0x004A, 0x004B, true),
        SpiFlag::MinimizeAnimation => (SPI_GETANIMATION, SPI_SETANIMATION, false),
    }
}

const PERSIST: u32 = SPIF_UPDATEINIFILE.0 | SPIF_SENDCHANGE.0;

fn spi(action: u32, ui: u32, pv: *mut c_void, persist: bool, op: &'static str) -> SysResult<()> {
    let flags =
        windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(if persist {
            PERSIST
        } else {
            0
        });
    // SAFETY: callers pass a pointer valid for the given action.
    unsafe { SystemParametersInfoW(SYSTEM_PARAMETERS_INFO_ACTION(action), ui, Some(pv), flags) }
        .map_err(|e| sys_err(op, format!("SPI 0x{action:04X}"), &e))
}

fn reg_string_u32(reg: &dyn Registry, path: &str, name: &str) -> SysResult<Option<u32>> {
    Ok(match reg.get(Hive::CurrentUser, path, name)? {
        Some(RegData::Sz(s)) => s.trim().parse().ok(),
        Some(RegData::Dword(v)) => Some(v),
        _ => None,
    })
}

pub struct WinDesktop;

impl WinDesktop {
    fn registry() -> super::registry::WinRegistry {
        super::registry::WinRegistry::new()
    }
}

impl Desktop for WinDesktop {
    fn spi_get(&self, flag: SpiFlag) -> SysResult<bool> {
        let (get, _, _) = codes(flag);
        if flag == SpiFlag::MinimizeAnimation {
            let mut info = ANIMATIONINFO {
                cbSize: size_of::<ANIMATIONINFO>() as u32,
                iMinAnimate: 0,
            };
            spi(
                get,
                info.cbSize,
                (&mut info as *mut ANIMATIONINFO).cast(),
                false,
                "read desktop setting",
            )?;
            return Ok(info.iMinAnimate != 0);
        }
        let mut value: i32 = 0;
        spi(
            get,
            0,
            (&mut value as *mut i32).cast(),
            false,
            "read desktop setting",
        )?;
        Ok(value != 0)
    }

    fn spi_set(&self, flag: SpiFlag, value: bool) -> SysResult<()> {
        let (_, set, via_ui) = codes(flag);
        if flag == SpiFlag::MinimizeAnimation {
            let mut info = ANIMATIONINFO {
                cbSize: size_of::<ANIMATIONINFO>() as u32,
                iMinAnimate: i32::from(value),
            };
            return spi(
                set,
                info.cbSize,
                (&mut info as *mut ANIMATIONINFO).cast(),
                true,
                "change desktop setting",
            );
        }
        if via_ui {
            spi(
                set,
                u32::from(value),
                std::ptr::null_mut(),
                true,
                "change desktop setting",
            )
        } else {
            // These actions take the BOOL itself in pvParam.
            spi(
                set,
                0,
                usize::from(value) as *mut c_void,
                true,
                "change desktop setting",
            )
        }
    }

    fn reload_mouse(&self) -> SysResult<()> {
        let reg = Self::registry();
        let key = r"Control Panel\Mouse";
        let mut values = [
            reg_string_u32(&reg, key, "MouseThreshold1")?.unwrap_or(6) as i32,
            reg_string_u32(&reg, key, "MouseThreshold2")?.unwrap_or(10) as i32,
            reg_string_u32(&reg, key, "MouseSpeed")?.unwrap_or(1) as i32,
        ];
        spi(
            SPI_SETMOUSE,
            0,
            values.as_mut_ptr().cast(),
            true,
            "apply mouse settings",
        )
    }

    fn reload_accessibility(&self) -> SysResult<()> {
        let reg = Self::registry();
        let base = r"Control Panel\Accessibility";
        if let Some(flags) = reg_string_u32(&reg, &format!(r"{base}\StickyKeys"), "Flags")? {
            let mut s = STICKYKEYS {
                cbSize: size_of::<STICKYKEYS>() as u32,
                ..Default::default()
            };
            spi(
                SPI_GETSTICKYKEYS,
                s.cbSize,
                (&mut s as *mut STICKYKEYS).cast(),
                false,
                "read Sticky Keys",
            )?;
            s.dwFlags = windows::Win32::UI::Accessibility::STICKYKEYS_FLAGS(flags);
            spi(
                SPI_SETSTICKYKEYS,
                s.cbSize,
                (&mut s as *mut STICKYKEYS).cast(),
                true,
                "apply Sticky Keys",
            )?;
        }
        if let Some(flags) = reg_string_u32(&reg, &format!(r"{base}\ToggleKeys"), "Flags")? {
            let mut t = TOGGLEKEYS {
                cbSize: size_of::<TOGGLEKEYS>() as u32,
                ..Default::default()
            };
            spi(
                SPI_GETTOGGLEKEYS,
                t.cbSize,
                (&mut t as *mut TOGGLEKEYS).cast(),
                false,
                "read Toggle Keys",
            )?;
            t.dwFlags = flags;
            spi(
                SPI_SETTOGGLEKEYS,
                t.cbSize,
                (&mut t as *mut TOGGLEKEYS).cast(),
                true,
                "apply Toggle Keys",
            )?;
        }
        if let Some(flags) = reg_string_u32(&reg, &format!(r"{base}\Keyboard Response"), "Flags")? {
            let mut f = FILTERKEYS {
                cbSize: size_of::<FILTERKEYS>() as u32,
                ..Default::default()
            };
            spi(
                SPI_GETFILTERKEYS,
                f.cbSize,
                (&mut f as *mut FILTERKEYS).cast(),
                false,
                "read Filter Keys",
            )?;
            f.dwFlags = flags;
            spi(
                SPI_SETFILTERKEYS,
                f.cbSize,
                (&mut f as *mut FILTERKEYS).cast(),
                true,
                "apply Filter Keys",
            )?;
        }
        Ok(())
    }

    fn broadcast_settings_change(&self) -> SysResult<()> {
        for area in ["ImmersiveColorSet", "Policy", "WindowsThemeElement"] {
            let area = HSTRING::from(area);
            // SAFETY: broadcast with a valid string; times out rather than hanging on a stuck window.
            unsafe {
                SendMessageTimeoutW(
                    HWND_BROADCAST,
                    WM_SETTINGCHANGE,
                    WPARAM(0),
                    LPARAM(area.as_ptr() as isize),
                    SMTO_ABORTIFHUNG,
                    2000,
                    None,
                )
            };
        }
        Ok(())
    }

    fn displays(&self) -> SysResult<Vec<DisplayInfo>> {
        let mut out = Vec::new();
        for i in 0.. {
            let mut dd = DISPLAY_DEVICEW {
                cb: size_of::<DISPLAY_DEVICEW>() as u32,
                ..Default::default()
            };
            // SAFETY: `dd` is sized and zeroed; enumeration ends when this fails.
            if !unsafe { EnumDisplayDevicesW(PCWSTR::null(), i, &mut dd, 0) }.as_bool() {
                break;
            }
            if (dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP).0 == 0 {
                continue;
            }
            let device = String::from_utf16_lossy(&dd.DeviceName)
                .trim_end_matches('\0')
                .to_owned();
            let name = HSTRING::from(device.as_str());

            let mut monitor = DISPLAY_DEVICEW {
                cb: size_of::<DISPLAY_DEVICEW>() as u32,
                ..Default::default()
            };
            // SAFETY: as above, for the monitor attached to this adapter output.
            let monitor_name =
                if unsafe { EnumDisplayDevicesW(&name, 0, &mut monitor, 0) }.as_bool() {
                    String::from_utf16_lossy(&monitor.DeviceString)
                        .trim_end_matches('\0')
                        .to_owned()
                } else {
                    device.clone()
                };

            let mut current = DEVMODEW {
                dmSize: size_of::<DEVMODEW>() as u16,
                ..Default::default()
            };
            // SAFETY: `current` is sized; ENUM_CURRENT_SETTINGS reads the active mode.
            if !unsafe { EnumDisplaySettingsW(&name, ENUM_CURRENT_SETTINGS, &mut current) }
                .as_bool()
            {
                continue;
            }
            let mut max_hz = current.dmDisplayFrequency;
            for mode in 0.. {
                let mut dm = DEVMODEW {
                    dmSize: size_of::<DEVMODEW>() as u16,
                    ..Default::default()
                };
                // SAFETY: enumerating modes until the call fails.
                if !unsafe {
                    EnumDisplaySettingsW(&name, ENUM_DISPLAY_SETTINGS_MODE(mode), &mut dm)
                }
                .as_bool()
                {
                    break;
                }
                if dm.dmPelsWidth == current.dmPelsWidth
                    && dm.dmPelsHeight == current.dmPelsHeight
                    && dm.dmBitsPerPel == current.dmBitsPerPel
                {
                    max_hz = max_hz.max(dm.dmDisplayFrequency);
                }
            }
            out.push(DisplayInfo {
                device,
                monitor: monitor_name,
                width: current.dmPelsWidth,
                height: current.dmPelsHeight,
                hz: current.dmDisplayFrequency,
                max_hz,
            });
        }
        Ok(out)
    }

    fn set_refresh(&self, device: &str, hz: u32) -> SysResult<()> {
        let name = HSTRING::from(device);
        let mut dm = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        // SAFETY: reads the current mode into a sized DEVMODEW.
        if !unsafe { EnumDisplaySettingsW(&name, ENUM_CURRENT_SETTINGS, &mut dm) }.as_bool() {
            return Err(SysError::new(
                SysErrorKind::NotFound,
                "change refresh rate",
                device,
                "Display not found.",
            ));
        }
        dm.dmDisplayFrequency = hz;
        dm.dmFields = DM_DISPLAYFREQUENCY;
        // SAFETY: valid device name and DEVMODEW.
        let result =
            unsafe { ChangeDisplaySettingsExW(&name, Some(&dm), None, CDS_UPDATEREGISTRY, None) };
        if result == DISP_CHANGE_SUCCESSFUL {
            Ok(())
        } else {
            Err(SysError::new(
                SysErrorKind::Other,
                "change refresh rate",
                device,
                format!("Windows refused {hz} Hz (code {}).", result.0),
            ))
        }
    }
}
