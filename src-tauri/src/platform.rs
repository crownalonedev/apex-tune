//! Small process- and OS-level facts the rest of the app needs.

use std::path::Path;

/// True when the current process token is elevated.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = HANDLE::default();
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no cleanup;
    // `token` is a valid out-pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    // SAFETY: the buffer is a TOKEN_ELEVATION and its exact size is passed.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    }
    .is_ok();
    // SAFETY: `token` was opened above and is closed exactly once.
    let _ = unsafe { CloseHandle(token) };
    ok && elevation.TokenIsElevated != 0
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Replaces the folder's DACL with: SYSTEM and Administrators full control,
/// Users read/execute, inherited by children, not inheriting from the parent.
#[cfg(windows)]
pub fn restrict_to_admins(dir: &Path) -> std::io::Result<()> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
        SetNamedSecurityInfoW,
    };
    use windows::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    };
    use windows::core::{HSTRING, PCWSTR};

    const SDDL: &str = "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)";
    let to_io = |e: windows::core::Error| std::io::Error::other(e.message());

    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: SDDL is a valid NUL-terminated wide string; `sd` receives a
    // LocalAlloc'd descriptor that is freed below.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            &HSTRING::from(SDDL),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
    }
    .map_err(to_io)?;

    let mut present = Default::default();
    let mut defaulted = Default::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    // SAFETY: `sd` is a valid descriptor; the DACL pointer borrows from it and
    // is only used before `sd` is freed.
    let result = unsafe { GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted) }
        .map_err(to_io)
        .and_then(|()| {
            let path = HSTRING::from(dir.as_os_str());
            // SAFETY: path and DACL are valid for the duration of the call.
            unsafe {
                SetNamedSecurityInfoW(
                    PCWSTR(path.as_ptr()),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    Some(dacl),
                    None,
                )
            }
            .ok()
            .map_err(to_io)
        });

    // SAFETY: `sd` was allocated by ConvertStringSecurityDescriptor... with LocalAlloc.
    unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    result
}

#[cfg(not(windows))]
pub fn restrict_to_admins(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}
