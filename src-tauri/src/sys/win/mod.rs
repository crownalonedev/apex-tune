//! Real Windows implementations of the `sys` traits.

pub mod power;
pub mod registry;
pub mod restore;
pub mod services;
pub mod tasks;

use super::{SysError, SysErrorKind};

const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_PATH_NOT_FOUND: u32 = 3;
const ERROR_ACCESS_DENIED: u32 = 5;
const ERROR_SERVICE_DOES_NOT_EXIST: u32 = 1060;

/// The Win32 error code inside an HRESULT_FROM_WIN32, if it is one.
pub(crate) fn win32_code(e: &windows::core::Error) -> Option<u32> {
    let hr = e.code().0 as u32;
    (hr & 0xFFFF_0000 == 0x8007_0000).then_some(hr & 0xFFFF)
}

pub(crate) fn is_not_found(e: &windows::core::Error) -> bool {
    matches!(
        win32_code(e),
        Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | ERROR_SERVICE_DOES_NOT_EXIST)
    )
}

pub(crate) fn sys_err(
    op: &'static str,
    target: impl Into<String>,
    e: &windows::core::Error,
) -> SysError {
    let kind = match win32_code(e) {
        Some(ERROR_ACCESS_DENIED) => SysErrorKind::AccessDenied,
        Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | ERROR_SERVICE_DOES_NOT_EXIST) => {
            SysErrorKind::NotFound
        }
        _ => SysErrorKind::Other,
    };
    let message = e.message();
    let message = if message.is_empty() {
        format!("error 0x{:08X}", e.code().0)
    } else {
        message
    };
    SysError::new(kind, op, target, message.trim_end().to_owned())
}
