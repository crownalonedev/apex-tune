use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountNameW, PSID, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::System::RemoteDesktop::{
    WTS_CURRENT_SERVER_HANDLE, WTS_INFO_CLASS, WTSDomainName, WTSFreeMemory,
    WTSGetActiveConsoleSessionId, WTSQuerySessionInformationW, WTSUserName,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows_registry::{Key, LOCAL_MACHINE, Type, USERS, Value};

use super::{is_not_found, sys_err};
use crate::sys::{Hive, RegData, Registry, SysResult};

/// Where `Hive::CurrentUser` points.
enum UserRoot {
    CurrentUser,
    /// `HKEY_USERS\<sid>` of the signed-in user, when Apex Tune was elevated
    /// with a different account than the one at the console.
    Users(String),
}

pub struct WinRegistry {
    user_root: UserRoot,
}

impl WinRegistry {
    pub fn new() -> Self {
        let user_root = match (interactive_user_sid(), process_user_sid()) {
            (Some(console), Some(process)) if !console.eq_ignore_ascii_case(&process) => {
                if USERS.open(&console).is_ok() {
                    tracing::info!(
                        "Elevated as a different account; user settings go to the signed-in user {console}"
                    );
                    UserRoot::Users(console)
                } else {
                    tracing::warn!(
                        "Signed-in user's hive {console} isn't loaded; user settings go to the elevated account"
                    );
                    UserRoot::CurrentUser
                }
            }
            _ => UserRoot::CurrentUser,
        };
        Self { user_root }
    }

    /// The root key and the path relative to it.
    fn locate(&self, hive: Hive, path: &str) -> (&'static Key, String) {
        let path = path.trim_matches('\\');
        match (hive, &self.user_root) {
            (Hive::LocalMachine, _) => (LOCAL_MACHINE, path.to_owned()),
            (Hive::CurrentUser, UserRoot::CurrentUser) => {
                (windows_registry::CURRENT_USER, path.to_owned())
            }
            (Hive::CurrentUser, UserRoot::Users(sid)) => (USERS, format!(r"{sid}\{path}")),
        }
    }

    fn open(&self, hive: Hive, path: &str) -> SysResult<Option<Key>> {
        let (root, rel) = self.locate(hive, path);
        match root.open(&rel) {
            Ok(key) => Ok(Some(key)),
            Err(e) if is_not_found(&e) => Ok(None),
            Err(e) => Err(sys_err("open registry key", format!(r"{hive}\{path}"), &e)),
        }
    }

    fn open_writable(&self, hive: Hive, path: &str, create: bool) -> SysResult<Option<Key>> {
        let (root, rel) = self.locate(hive, path);
        let mut options = root.options();
        options.read().write();
        if create {
            options.create();
        }
        match options.open(&rel) {
            Ok(key) => Ok(Some(key)),
            Err(e) if !create && is_not_found(&e) => Ok(None),
            Err(e) => Err(sys_err(
                "open registry key for writing",
                format!(r"{hive}\{path}"),
                &e,
            )),
        }
    }
}

impl Default for WinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn to_reg_data(value: Value) -> RegData {
    match value.ty() {
        Type::U32 if value.len() >= 4 => {
            RegData::Dword(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
        }
        Type::U64 if value.len() >= 8 => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&value[..8]);
            RegData::Qword(u64::from_le_bytes(b))
        }
        Type::String => RegData::Sz(wide_to_string(&value)),
        Type::ExpandString => RegData::ExpandSz(wide_to_string(&value)),
        Type::MultiString => RegData::MultiSz(
            wide_to_string_raw(&value)
                .split('\0')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        Type::Bytes => RegData::Binary(value.to_vec()),
        Type::Other(ty) => RegData::Other {
            ty,
            bytes: value.to_vec(),
        },
        // Malformed DWORD/QWORD: keep the raw bytes so a restore is exact.
        Type::U32 => RegData::Other {
            ty: 4,
            bytes: value.to_vec(),
        },
        Type::U64 => RegData::Other {
            ty: 11,
            bytes: value.to_vec(),
        },
    }
}

fn wide_to_string_raw(bytes: &[u8]) -> String {
    let wide: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16_lossy(&wide)
}

fn wide_to_string(bytes: &[u8]) -> String {
    wide_to_string_raw(bytes).trim_end_matches('\0').to_owned()
}

impl Registry for WinRegistry {
    fn get(&self, hive: Hive, path: &str, name: &str) -> SysResult<Option<RegData>> {
        let Some(key) = self.open(hive, path)? else {
            return Ok(None);
        };
        match key.get_value(name) {
            Ok(value) => Ok(Some(to_reg_data(value))),
            Err(e) if is_not_found(&e) => Ok(None),
            Err(e) => Err(sys_err(
                "read registry value",
                format!(r"{hive}\{path}\{name}"),
                &e,
            )),
        }
    }

    fn key_exists(&self, hive: Hive, path: &str) -> SysResult<bool> {
        Ok(self.open(hive, path)?.is_some())
    }

    fn set(&self, hive: Hive, path: &str, name: &str, data: &RegData) -> SysResult<()> {
        let target = format!(r"{hive}\{path}\{name}");
        let key = self
            .open_writable(hive, path, true)?
            .expect("create always returns a key");
        let result = match data {
            RegData::Dword(v) => key.set_u32(name, *v),
            RegData::Qword(v) => key.set_u64(name, *v),
            RegData::Sz(s) => key.set_string(name, s),
            RegData::ExpandSz(s) => key.set_expand_string(name, s),
            RegData::MultiSz(items) => {
                let items: Vec<&str> = items.iter().map(String::as_str).collect();
                key.set_multi_string(name, &items)
            }
            RegData::Binary(bytes) => key.set_bytes(name, Type::Bytes, bytes),
            RegData::Other { ty, bytes } => key.set_bytes(name, Type::Other(*ty), bytes),
        };
        result.map_err(|e| sys_err("write registry value", target, &e))
    }

    fn delete_value(&self, hive: Hive, path: &str, name: &str) -> SysResult<()> {
        let Some(key) = self.open_writable(hive, path, false)? else {
            return Ok(());
        };
        match key.remove_value(name) {
            Ok(()) => Ok(()),
            Err(e) if is_not_found(&e) => Ok(()),
            Err(e) => Err(sys_err(
                "delete registry value",
                format!(r"{hive}\{path}\{name}"),
                &e,
            )),
        }
    }

    fn delete_key_if_empty(&self, hive: Hive, path: &str) -> SysResult<bool> {
        let target = format!(r"{hive}\{path}");
        let Some(key) = self.open(hive, path)? else {
            return Ok(false);
        };
        let has_subkeys = key
            .keys()
            .map_err(|e| sys_err("list registry keys", &target, &e))?
            .next()
            .is_some();
        let has_values = key
            .values()
            .map_err(|e| sys_err("list registry values", &target, &e))?
            .next()
            .is_some();
        drop(key);
        if has_subkeys || has_values {
            return Ok(false);
        }
        let trimmed = path.trim_matches('\\');
        let (parent, leaf) = trimmed.rsplit_once('\\').unwrap_or(("", trimmed));
        let (root, rel_parent) = self.locate(hive, parent);
        let parent_key = if rel_parent.is_empty() {
            root.options().read().write().open("")
        } else {
            root.options().read().write().open(&rel_parent)
        }
        .map_err(|e| sys_err("open registry key for writing", &target, &e))?;
        // The key was just checked to be empty, so this removes only that key.
        parent_key
            .remove_tree(leaf)
            .map_err(|e| sys_err("delete registry key", &target, &e))?;
        Ok(true)
    }

    fn subkeys(&self, hive: Hive, path: &str) -> SysResult<Vec<String>> {
        let Some(key) = self.open(hive, path)? else {
            return Ok(Vec::new());
        };
        Ok(key
            .keys()
            .map_err(|e| sys_err("list registry keys", format!(r"{hive}\{path}"), &e))?
            .collect())
    }
}

// ---- Who is signed in -----------------------------------------------------

fn sid_to_string(sid: PSID) -> Option<String> {
    let mut out = PWSTR::null();
    // SAFETY: `sid` points to a valid SID; `out` receives a LocalAlloc'd string freed below.
    unsafe { ConvertSidToStringSidW(sid, &mut out) }.ok()?;
    // SAFETY: `out` is a valid NUL-terminated string from the call above.
    let s = unsafe { out.to_string() }.ok();
    // SAFETY: allocated by ConvertSidToStringSidW with LocalAlloc.
    unsafe { LocalFree(Some(HLOCAL(out.0.cast()))) };
    s
}

fn process_user_sid() -> Option<String> {
    let mut token = HANDLE::default();
    // SAFETY: pseudo-handle for this process; `token` is a valid out-pointer.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.ok()?;
    let mut needed = 0u32;
    // SAFETY: size query with no buffer; expected to fail with the needed size.
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut needed) };
    let mut buf = vec![0u8; needed as usize];
    // SAFETY: `buf` has exactly `needed` bytes.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )
    };
    // SAFETY: `token` was opened above.
    let _ = unsafe { CloseHandle(token) };
    ok.ok()?;
    // SAFETY: on success the buffer holds a TOKEN_USER whose SID points into the buffer.
    let user = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
    sid_to_string(user.User.Sid)
}

fn session_string(session: u32, class: WTS_INFO_CLASS) -> Option<String> {
    let mut buf = PWSTR::null();
    let mut len = 0u32;
    // SAFETY: valid out-pointers; the buffer is freed with WTSFreeMemory below.
    unsafe {
        WTSQuerySessionInformationW(
            Some(WTS_CURRENT_SERVER_HANDLE),
            session,
            class,
            &mut buf,
            &mut len,
        )
    }
    .ok()?;
    // SAFETY: WTS returns a NUL-terminated wide string.
    let s = unsafe { buf.to_string() }.ok();
    // SAFETY: allocated by WTSQuerySessionInformationW.
    unsafe { WTSFreeMemory(buf.0.cast()) };
    s.filter(|s| !s.is_empty())
}

fn interactive_user_sid() -> Option<String> {
    // SAFETY: no arguments; returns 0xFFFFFFFF when there's no console session.
    let session = unsafe { WTSGetActiveConsoleSessionId() };
    if session == u32::MAX {
        return None;
    }
    let account = HSTRING::from(format!(
        r"{}\{}",
        session_string(session, WTSDomainName)?,
        session_string(session, WTSUserName)?
    ));

    let mut sid_len = 0u32;
    let mut domain_len = 0u32;
    let mut use_ = SID_NAME_USE::default();
    // SAFETY: size query; expected to fail with the needed sizes.
    let _ = unsafe {
        LookupAccountNameW(
            PCWSTR::null(),
            &account,
            None,
            &mut sid_len,
            None,
            &mut domain_len,
            &mut use_,
        )
    };
    let mut sid = vec![0u8; sid_len as usize];
    let mut domain = vec![0u16; domain_len as usize];
    // SAFETY: buffers sized by the query above.
    unsafe {
        LookupAccountNameW(
            PCWSTR::null(),
            &account,
            Some(PSID(sid.as_mut_ptr().cast())),
            &mut sid_len,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domain_len,
            &mut use_,
        )
    }
    .ok()?;
    sid_to_string(PSID(sid.as_mut_ptr().cast()))
}
