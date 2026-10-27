//! Starting and stopping the apex-timer tray helper.

use std::path::PathBuf;

use crate::error::{AppError, AppResult};

/// The helper ships next to apex-tune.exe.
pub fn exe_path() -> Option<PathBuf> {
    let exe = std::env::current_exe()
        .ok()?
        .parent()?
        .join(apex_timer::EXE_NAME);
    // Debug builds may carry an empty placeholder (see build.rs); that's not a helper.
    std::fs::metadata(&exe)
        .is_ok_and(|m| m.len() > 0)
        .then_some(exe)
}

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS, EVENT_MODIFY_STATE, OpenEventW, OpenMutexW,
        SYNCHRONIZATION_SYNCHRONIZE, SetEvent,
    };
    use windows::core::HSTRING;

    use super::*;

    pub fn running() -> bool {
        // SAFETY: valid name; the handle is closed right away.
        match unsafe {
            OpenMutexW(
                SYNCHRONIZATION_SYNCHRONIZE,
                false,
                &HSTRING::from(apex_timer::MUTEX_NAME),
            )
        } {
            Ok(h) => {
                // SAFETY: handle from OpenMutexW.
                let _ = unsafe { CloseHandle(h) };
                true
            }
            Err(_) => false,
        }
    }

    fn wait_until(want_running: bool, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if running() == want_running {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        running() == want_running
    }

    pub fn start(units: u32) -> AppResult<()> {
        let exe = exe_path().ok_or_else(|| {
            AppError::Other(format!(
                "{} is missing from the Apex Tune folder. Reinstall Apex Tune.",
                apex_timer::EXE_NAME
            ))
        })?;
        stop()?;
        std::process::Command::new(exe)
            .args(["--resolution", &units.to_string()])
            .creation_flags((DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP).0)
            .spawn()?;
        if wait_until(true, Duration::from_secs(3)) {
            Ok(())
        } else {
            Err(AppError::Other(
                "The timer helper didn't start. Check the activity log.".into(),
            ))
        }
    }

    pub fn stop() -> AppResult<()> {
        if !running() {
            return Ok(());
        }
        // SAFETY: valid name; the handle is closed after signalling.
        let event = unsafe {
            OpenEventW(
                EVENT_MODIFY_STATE,
                false,
                &HSTRING::from(apex_timer::STOP_EVENT_NAME),
            )
        }?;
        // SAFETY: valid event handle.
        let result = unsafe { SetEvent(event) };
        // SAFETY: handle from OpenEventW.
        let _ = unsafe { CloseHandle(event) };
        result?;
        if wait_until(false, Duration::from_secs(3)) {
            Ok(())
        } else {
            Err(AppError::Other(
                "The timer helper didn't stop. Use Stop in its tray menu.".into(),
            ))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn running() -> bool {
        false
    }

    pub fn start(_units: u32) -> AppResult<()> {
        Err(AppError::Other(
            "The timer helper only runs on Windows.".into(),
        ))
    }

    pub fn stop() -> AppResult<()> {
        Ok(())
    }
}

pub use imp::{running, start, stop};
