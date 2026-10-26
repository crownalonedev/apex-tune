//! Power plans through powrprof (the API behind `powercfg`).

use windows::Win32::Foundation::{ERROR_SUCCESS, HLOCAL, LocalFree, WIN32_ERROR};
use windows::Win32::System::Power::{
    ACCESS_SCHEME, GetSystemPowerStatus, PowerDeleteScheme, PowerDuplicateScheme, PowerEnumerate,
    PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex, PowerReadFriendlyName,
    PowerSetActiveScheme, PowerWriteACValueIndex, PowerWriteDCValueIndex, PowerWriteDescription,
    PowerWriteFriendlyName, SYSTEM_POWER_STATUS,
};
use windows::core::GUID;

use crate::sys::{Power, PowerScheme, PowerSource, SysError, SysErrorKind, SysResult};

const ERROR_FILE_NOT_FOUND: u32 = 2;
/// BatteryFlag value meaning "no system battery".
const NO_BATTERY: u8 = 128;

fn parse(op: &'static str, guid: &str) -> SysResult<GUID> {
    let hex: String = guid.chars().filter(|c| *c != '-').collect();
    u128::from_str_radix(&hex, 16)
        .ok()
        .filter(|_| hex.len() == 32)
        .map(GUID::from_u128)
        .ok_or_else(|| SysError::new(SysErrorKind::Other, op, guid, "not a valid GUID"))
}

pub(crate) fn format_guid(g: &GUID) -> String {
    format!("{:032x}", g.to_u128())
        .char_indices()
        .flat_map(|(i, c)| {
            (matches!(i, 8 | 12 | 16 | 20))
                .then_some('-')
                .into_iter()
                .chain([c])
        })
        .collect()
}

fn check(op: &'static str, target: &str, code: WIN32_ERROR) -> SysResult<()> {
    if code == ERROR_SUCCESS {
        return Ok(());
    }
    let kind = match code.0 {
        5 => SysErrorKind::AccessDenied,
        ERROR_FILE_NOT_FOUND => SysErrorKind::NotFound,
        _ => SysErrorKind::Other,
    };
    let message = windows::core::Error::from(code.to_hresult()).message();
    Err(SysError::new(
        kind,
        op,
        target,
        message.trim_end().to_owned(),
    ))
}

fn friendly_name(scheme: &GUID) -> String {
    let mut size = 0u32;
    // SAFETY: size query with no buffer.
    let _ = unsafe { PowerReadFriendlyName(None, Some(scheme), None, None, None, &mut size) };
    if size == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: the buffer holds `size` bytes.
    let code = unsafe {
        PowerReadFriendlyName(
            None,
            Some(scheme),
            None,
            None,
            Some(buf.as_mut_ptr().cast()),
            &mut size,
        )
    };
    if code != ERROR_SUCCESS {
        return String::new();
    }
    String::from_utf16_lossy(&buf)
        .trim_end_matches('\0')
        .to_owned()
}

fn wide_bytes(s: &str) -> Vec<u8> {
    s.encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect()
}

pub struct WinPower;

impl Power for WinPower {
    fn schemes(&self) -> SysResult<Vec<PowerScheme>> {
        let mut schemes = Vec::new();
        for index in 0.. {
            let mut guid = GUID::zeroed();
            let mut size = size_of::<GUID>() as u32;
            // SAFETY: `guid` is a buffer of `size` bytes.
            let code = unsafe {
                PowerEnumerate(
                    None,
                    None,
                    None,
                    ACCESS_SCHEME,
                    index,
                    Some((&mut guid as *mut GUID).cast()),
                    &mut size,
                )
            };
            if code != ERROR_SUCCESS {
                break; // ERROR_NO_MORE_ITEMS
            }
            schemes.push(PowerScheme {
                guid: format_guid(&guid),
                name: friendly_name(&guid),
            });
        }
        Ok(schemes)
    }

    fn active(&self) -> SysResult<String> {
        let mut ptr: *mut GUID = std::ptr::null_mut();
        // SAFETY: `ptr` receives a LocalAlloc'd GUID freed below.
        check("read active power plan", "", unsafe {
            PowerGetActiveScheme(None, &mut ptr)
        })?;
        // SAFETY: set by the successful call.
        let guid = unsafe { *ptr };
        // SAFETY: allocated by PowerGetActiveScheme.
        unsafe { LocalFree(Some(HLOCAL(ptr.cast()))) };
        Ok(format_guid(&guid))
    }

    fn set_active(&self, guid: &str) -> SysResult<()> {
        let g = parse("switch power plan", guid)?;
        // SAFETY: valid GUID pointer.
        check("switch power plan", guid, unsafe {
            PowerSetActiveScheme(None, Some(&g))
        })
    }

    fn exists(&self, guid: &str) -> SysResult<bool> {
        Ok(self
            .schemes()?
            .iter()
            .any(|s| s.guid.eq_ignore_ascii_case(guid)))
    }

    fn duplicate(&self, source: &str, dest: &str) -> SysResult<()> {
        let src = parse("create power plan", source)?;
        let mut dst = parse("create power plan", dest)?;
        let mut dst_ptr: *mut GUID = &mut dst;
        // SAFETY: a non-null destination pointer tells powrprof to use that GUID.
        check("create power plan", source, unsafe {
            PowerDuplicateScheme(None, &src, &mut dst_ptr)
        })
    }

    fn delete(&self, guid: &str) -> SysResult<()> {
        let g = parse("delete power plan", guid)?;
        // SAFETY: valid GUID pointer.
        check("delete power plan", guid, unsafe {
            PowerDeleteScheme(None, &g)
        })
    }

    fn rename(&self, guid: &str, name: &str, description: &str) -> SysResult<()> {
        let g = parse("rename power plan", guid)?;
        // SAFETY: valid GUID and NUL-terminated UTF-16 buffers.
        check("rename power plan", guid, unsafe {
            PowerWriteFriendlyName(None, &g, None, None, &wide_bytes(name))
        })?;
        // SAFETY: as above.
        check("rename power plan", guid, unsafe {
            PowerWriteDescription(None, &g, None, None, &wide_bytes(description))
        })
    }

    fn read(
        &self,
        scheme: &str,
        subgroup: &str,
        setting: &str,
        source: PowerSource,
    ) -> SysResult<Option<u32>> {
        let (s, sub, set) = (
            parse("read power setting", scheme)?,
            parse("read power setting", subgroup)?,
            parse("read power setting", setting)?,
        );
        let mut value = 0u32;
        // SAFETY: valid GUID pointers and out-pointer.
        let code = unsafe {
            match source {
                PowerSource::Ac => {
                    PowerReadACValueIndex(None, Some(&s), Some(&sub), Some(&set), &mut value)
                }
                PowerSource::Dc => WIN32_ERROR(PowerReadDCValueIndex(
                    None,
                    Some(&s),
                    Some(&sub),
                    Some(&set),
                    &mut value,
                )),
            }
        };
        if code.0 == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check("read power setting", setting, code)?;
        Ok(Some(value))
    }

    fn write(
        &self,
        scheme: &str,
        subgroup: &str,
        setting: &str,
        source: PowerSource,
        value: u32,
    ) -> SysResult<()> {
        let (s, sub, set) = (
            parse("write power setting", scheme)?,
            parse("write power setting", subgroup)?,
            parse("write power setting", setting)?,
        );
        // SAFETY: valid GUID pointers.
        let code = unsafe {
            match source {
                PowerSource::Ac => PowerWriteACValueIndex(None, &s, Some(&sub), Some(&set), value),
                PowerSource::Dc => WIN32_ERROR(PowerWriteDCValueIndex(
                    None,
                    &s,
                    Some(&sub),
                    Some(&set),
                    value,
                )),
            }
        };
        check("write power setting", setting, code)?;
        // Values written to the active plan only take effect once it's re-applied.
        if self.active()?.eq_ignore_ascii_case(scheme) {
            self.set_active(scheme)?;
        }
        Ok(())
    }

    fn has_battery(&self) -> SysResult<bool> {
        let mut status = SYSTEM_POWER_STATUS::default();
        // SAFETY: valid out-pointer.
        unsafe { GetSystemPowerStatus(&mut status) }
            .map_err(|e| super::sys_err("read power status", "battery", &e))?;
        Ok(status.BatteryFlag != NO_BATTERY && status.BatteryFlag != 255)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_round_trip() {
        let s = "381b4222-f694-41f0-9685-ff5bb260df2e";
        assert_eq!(format_guid(&parse("t", s).unwrap()), s);
    }
}
