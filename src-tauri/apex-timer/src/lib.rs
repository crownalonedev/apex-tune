//! Timer-resolution primitives shared by Apex Tune and its tray helper.
//!
//! Resolutions are in 100-nanosecond units, as the NT API uses them:
//! 5000 = 0.5 ms, 10000 = 1 ms, 156250 = 15.625 ms (the Windows default).

/// Held by the running helper; its presence means the helper is running.
pub const MUTEX_NAME: &str = r"Local\ApexTuneTimer";
/// Signalled by Apex Tune to ask the helper to exit.
pub const STOP_EVENT_NAME: &str = r"Local\ApexTuneTimerStop";
pub const EXE_NAME: &str = "apex-timer.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    /// Largest interval (least precise), usually 156250.
    pub coarsest: u32,
    /// Smallest interval the hardware supports, usually 5000.
    pub finest: u32,
    pub current: u32,
}

pub fn to_ms(units: u32) -> f64 {
    f64::from(units) / 10_000.0
}

/// Parses `--resolution <100ns units>`; anything else is an error message.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<u32, String> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--resolution" {
            let value = args.next().ok_or("--resolution needs a value")?;
            let units: u32 = value
                .parse()
                .map_err(|_| format!("not a number: {value}"))?;
            // 0.1 ms to 15.625 ms; anything outside is a mistake.
            return if (1_000..=156_250).contains(&units) {
                Ok(units)
            } else {
                Err(format!("resolution {units} is out of range"))
            };
        }
    }
    Err("usage: apex-timer --resolution <100ns units>".into())
}

#[cfg(windows)]
mod nt {
    #[link(name = "ntdll", kind = "raw-dylib")]
    unsafe extern "system" {
        pub fn NtQueryTimerResolution(
            coarsest: *mut u32,
            finest: *mut u32,
            current: *mut u32,
        ) -> i32;
        pub fn NtSetTimerResolution(desired: u32, set: u8, current: *mut u32) -> i32;
    }
}

#[cfg(windows)]
pub fn query() -> Option<Resolution> {
    let (mut coarsest, mut finest, mut current) = (0, 0, 0);
    // SAFETY: three valid out-pointers.
    let status = unsafe { nt::NtQueryTimerResolution(&mut coarsest, &mut finest, &mut current) };
    (status >= 0).then_some(Resolution {
        coarsest,
        finest,
        current,
    })
}

/// Requests a resolution for this process. Returns the resulting current resolution.
#[cfg(windows)]
pub fn request(units: u32) -> Option<u32> {
    let mut current = 0;
    // SAFETY: valid out-pointer.
    let status = unsafe { nt::NtSetTimerResolution(units, 1, &mut current) };
    (status >= 0).then_some(current)
}

/// Withdraws this process's request.
#[cfg(windows)]
pub fn release(units: u32) {
    let mut current = 0;
    // SAFETY: valid out-pointer.
    unsafe { nt::NtSetTimerResolution(units, 0, &mut current) };
}

/// Windows 11 ignores timer requests from processes it considers hidden or
/// inaudible. This opts the current process out of that throttling.
#[cfg(windows)]
pub fn honor_timer_requests_when_hidden() -> bool {
    use windows::Win32::System::Threading::{
        GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetProcessInformation,
    };
    const IGNORE_TIMER_RESOLUTION: u32 = 0x4;
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: IGNORE_TIMER_RESOLUTION,
        StateMask: 0,
    };
    // SAFETY: `state` is a valid PROCESS_POWER_THROTTLING_STATE of the given size.
    unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            (&state as *const PROCESS_POWER_THROTTLING_STATE).cast(),
            size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
    }
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn parses_resolution() {
        assert_eq!(parse_args(args("apex-timer --resolution 5000")), Ok(5000));
        assert!(parse_args(args("apex-timer --resolution 50")).is_err());
        assert!(parse_args(args("apex-timer --resolution x")).is_err());
        assert!(parse_args(args("apex-timer")).is_err());
    }

    #[test]
    fn converts_to_ms() {
        assert_eq!(to_ms(5000), 0.5);
        assert_eq!(to_ms(156_250), 15.625);
    }
}
