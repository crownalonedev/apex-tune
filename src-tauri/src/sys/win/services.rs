use std::time::{Duration, Instant};

use windows::Win32::System::Services::{
    ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ControlService,
    ENUM_SERVICE_TYPE, OpenSCManagerW, OpenServiceW, QUERY_SERVICE_CONFIGW, QueryServiceConfig2W,
    QueryServiceConfigW, QueryServiceStatus, SC_HANDLE, SC_MANAGER_CONNECT, SERVICE_ALL_ACCESS,
    SERVICE_AUTO_START, SERVICE_BOOT_START, SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
    SERVICE_CONTROL_STOP, SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DEMAND_START, SERVICE_DISABLED,
    SERVICE_ERROR, SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
    SERVICE_START_TYPE, SERVICE_STATUS, SERVICE_STOPPED, SERVICE_SYSTEM_START, StartServiceW,
};
use windows::core::{HSTRING, PCWSTR};

use super::{is_not_found, sys_err};
use crate::sys::{ServiceState, Services, StartType, SysError, SysErrorKind, SysResult};

const STOP_TIMEOUT: Duration = Duration::from_secs(20);

/// Closes a service handle on drop.
struct Handle(SC_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenSCManagerW/OpenServiceW and is closed once.
        let _ = unsafe { CloseServiceHandle(self.0) };
    }
}

fn open(name: &str, access: u32) -> SysResult<Option<Handle>> {
    // SAFETY: null machine and database names mean the local SCM.
    let scm = unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) }
        .map_err(|e| sys_err("open Service Control Manager", name, &e))?;
    let scm = Handle(scm);
    // SAFETY: `scm` is a valid handle for the duration of the call.
    match unsafe { OpenServiceW(scm.0, &HSTRING::from(name), access) } {
        Ok(h) => Ok(Some(Handle(h))),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(sys_err("open service", name, &e)),
    }
}

fn require(name: &str, access: u32, op: &'static str) -> SysResult<Handle> {
    open(name, access)?.ok_or_else(|| {
        SysError::new(
            SysErrorKind::NotFound,
            op,
            name,
            "The service isn't installed.",
        )
    })
}

fn status(h: &Handle, name: &str) -> SysResult<SERVICE_STATUS> {
    let mut status = SERVICE_STATUS::default();
    // SAFETY: valid handle and out-pointer.
    unsafe { QueryServiceStatus(h.0, &mut status) }
        .map_err(|e| sys_err("query service status", name, &e))?;
    Ok(status)
}

fn start_type(h: &Handle, name: &str) -> SysResult<StartType> {
    let mut needed = 0u32;
    // SAFETY: size query; expected to fail with ERROR_INSUFFICIENT_BUFFER.
    let _ = unsafe { QueryServiceConfigW(h.0, None, 0, &mut needed) };
    // u64 elements keep the buffer aligned for QUERY_SERVICE_CONFIGW.
    let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
    let config = buf.as_mut_ptr().cast::<QUERY_SERVICE_CONFIGW>();
    // SAFETY: buffer is at least `needed` bytes and suitably aligned.
    unsafe { QueryServiceConfigW(h.0, Some(config), needed, &mut needed) }
        .map_err(|e| sys_err("query service configuration", name, &e))?;
    // SAFETY: filled by the successful call above.
    let raw = unsafe { (*config).dwStartType };

    Ok(match raw {
        SERVICE_BOOT_START => StartType::Boot,
        SERVICE_SYSTEM_START => StartType::System,
        SERVICE_DEMAND_START => StartType::Manual,
        SERVICE_DISABLED => StartType::Disabled,
        SERVICE_AUTO_START if delayed(h, name)? => StartType::AutomaticDelayed,
        _ => StartType::Automatic,
    })
}

fn delayed(h: &Handle, name: &str) -> SysResult<bool> {
    let mut info = SERVICE_DELAYED_AUTO_START_INFO::default();
    let mut needed = 0u32;
    let bytes = size_of::<SERVICE_DELAYED_AUTO_START_INFO>();
    // SAFETY: `info` is exactly `bytes` long.
    let buf = unsafe {
        std::slice::from_raw_parts_mut(
            (&mut info as *mut SERVICE_DELAYED_AUTO_START_INFO).cast::<u8>(),
            bytes,
        )
    };
    // SAFETY: valid handle, buffer and out-pointer.
    unsafe {
        QueryServiceConfig2W(
            h.0,
            SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
            Some(buf),
            &mut needed,
        )
    }
    .map_err(|e| sys_err("query service delayed start", name, &e))?;
    Ok(info.fDelayedAutostart.as_bool())
}

pub struct WinServices;

impl Services for WinServices {
    fn query(&self, name: &str) -> SysResult<Option<ServiceState>> {
        let Some(h) = open(name, SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS)? else {
            return Ok(None);
        };
        let start = start_type(&h, name)?;
        let running = status(&h, name)?.dwCurrentState == SERVICE_RUNNING;
        Ok(Some(ServiceState { start, running }))
    }

    fn set_start(&self, name: &str, start: StartType) -> SysResult<()> {
        let h = require(name, SERVICE_ALL_ACCESS, "configure service")?;
        let raw: SERVICE_START_TYPE = match start {
            StartType::Boot => SERVICE_BOOT_START,
            StartType::System => SERVICE_SYSTEM_START,
            StartType::Automatic | StartType::AutomaticDelayed => SERVICE_AUTO_START,
            StartType::Manual => SERVICE_DEMAND_START,
            StartType::Disabled => SERVICE_DISABLED,
        };
        // SAFETY: valid handle; null pointers mean "leave unchanged".
        unsafe {
            ChangeServiceConfigW(
                h.0,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                raw,
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                PCWSTR::null(),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
            )
        }
        .map_err(|e| sys_err("configure service", name, &e))?;

        if matches!(start, StartType::Automatic | StartType::AutomaticDelayed) {
            let info = SERVICE_DELAYED_AUTO_START_INFO {
                fDelayedAutostart: (start == StartType::AutomaticDelayed).into(),
            };
            // SAFETY: `info` is a valid SERVICE_DELAYED_AUTO_START_INFO for the call.
            unsafe {
                ChangeServiceConfig2W(
                    h.0,
                    SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
                    Some((&info as *const SERVICE_DELAYED_AUTO_START_INFO).cast()),
                )
            }
            .map_err(|e| sys_err("configure service delayed start", name, &e))?;
        }
        Ok(())
    }

    fn stop(&self, name: &str) -> SysResult<()> {
        let h = require(name, SERVICE_ALL_ACCESS, "stop service")?;
        if status(&h, name)?.dwCurrentState == SERVICE_STOPPED {
            return Ok(());
        }
        let mut st = SERVICE_STATUS::default();
        // SAFETY: valid handle and out-pointer.
        unsafe { ControlService(h.0, SERVICE_CONTROL_STOP, &mut st) }
            .map_err(|e| sys_err("stop service", name, &e))?;
        let deadline = Instant::now() + STOP_TIMEOUT;
        while status(&h, name)?.dwCurrentState != SERVICE_STOPPED {
            if Instant::now() > deadline {
                return Err(SysError::new(
                    SysErrorKind::Other,
                    "stop service",
                    name,
                    "It didn't stop within 20 seconds.",
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }

    fn start(&self, name: &str) -> SysResult<()> {
        let h = require(name, SERVICE_ALL_ACCESS, "start service")?;
        if status(&h, name)?.dwCurrentState == SERVICE_RUNNING {
            return Ok(());
        }
        // SAFETY: valid handle, no arguments.
        unsafe { StartServiceW(h.0, None) }.map_err(|e| sys_err("start service", name, &e))
    }
}
