//! NVIDIA driver settings through NVAPI's DRS (driver settings) interface.
//!
//! nvapi64.dll ships with the NVIDIA driver and exposes one export,
//! `nvapi_QueryInterface`, which returns function pointers by ID. Struct
//! layouts and IDs follow NVIDIA's public headers (github.com/NVIDIA/nvapi);
//! function IDs match the open-source nvapi-rs bindings.

use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;

use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{HSTRING, s};

use crate::sys::{NvInfo, NvProfile, Nvidia, SysError, SysErrorKind, SysResult};

type Handle = *mut c_void;

const UNICODE_MAX: usize = 2048;
const NVAPI_OK: i32 = 0;
const NVAPI_SETTING_NOT_FOUND: i32 = -160;
const NVAPI_PROFILE_NOT_FOUND: i32 = -163;
const NVAPI_EXECUTABLE_NOT_FOUND: i32 = -166;
const CURRENT_PROFILE_LOCATION: u32 = 0;
const DWORD_TYPE: u32 = 0;
const THERMAL_TARGET_ALL: u32 = 15;

/// Profiles Apex Tune creates are named with this prefix, so only those are ever deleted.
const PROFILE_PREFIX: &str = "Apex Tune - ";

#[repr(C, packed(4))]
struct Setting {
    version: u32,
    name: [u16; UNICODE_MAX],
    id: u32,
    kind: u32,
    location: u32,
    is_current_predefined: u32,
    is_predefined_valid: u32,
    predefined: [u8; 4100],
    current: [u8; 4100],
}

#[repr(C)]
struct Profile {
    version: u32,
    name: [u16; UNICODE_MAX],
    gpu_support: u32,
    is_predefined: u32,
    num_apps: u32,
    num_settings: u32,
}

#[repr(C)]
struct Application {
    version: u32,
    is_predefined: u32,
    app_name: [u16; UNICODE_MAX],
    friendly_name: [u16; UNICODE_MAX],
    launcher: [u16; UNICODE_MAX],
    file_in_folder: [u16; UNICODE_MAX],
    flags: u32,
    command_line: [u16; UNICODE_MAX],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Sensor {
    controller: i32,
    default_min: i32,
    default_max: i32,
    current: i32,
    target: i32,
}

#[repr(C)]
#[derive(Default)]
struct Thermal {
    version: u32,
    count: u32,
    sensors: [Sensor; 3],
}

const fn version<T>(v: u32) -> u32 {
    size_of::<T>() as u32 | (v << 16)
}

fn zeroed<T>() -> Box<T> {
    // SAFETY: every struct here is plain integers and arrays, so all-zero is valid.
    unsafe { Box::new(std::mem::zeroed()) }
}

fn unicode(s: &str) -> [u16; UNICODE_MAX] {
    let mut out = [0u16; UNICODE_MAX];
    for (i, c) in s.encode_utf16().take(UNICODE_MAX - 1).enumerate() {
        out[i] = c;
    }
    out
}

struct Api {
    create_session: unsafe extern "C" fn(*mut Handle) -> i32,
    destroy_session: unsafe extern "C" fn(Handle) -> i32,
    load_settings: unsafe extern "C" fn(Handle) -> i32,
    save_settings: unsafe extern "C" fn(Handle) -> i32,
    get_base_profile: unsafe extern "C" fn(Handle, *mut Handle) -> i32,
    find_application:
        unsafe extern "C" fn(Handle, *const u16, *mut Handle, *mut Application) -> i32,
    find_profile: unsafe extern "C" fn(Handle, *const u16, *mut Handle) -> i32,
    create_profile: unsafe extern "C" fn(Handle, *mut Profile, *mut Handle) -> i32,
    delete_profile: unsafe extern "C" fn(Handle, Handle) -> i32,
    create_application: unsafe extern "C" fn(Handle, Handle, *mut Application) -> i32,
    get_setting: unsafe extern "C" fn(Handle, Handle, u32, *mut Setting) -> i32,
    set_setting: unsafe extern "C" fn(Handle, Handle, *mut Setting) -> i32,
    delete_setting: unsafe extern "C" fn(Handle, Handle, u32) -> i32,
    save_to_file: unsafe extern "C" fn(Handle, *const u16) -> i32,
    load_from_file: unsafe extern "C" fn(Handle, *const u16) -> i32,
    driver_version: unsafe extern "C" fn(*mut u32, *mut [u8; 64]) -> i32,
    enum_gpus: unsafe extern "C" fn(*mut [Handle; 64], *mut u32) -> i32,
    gpu_name: unsafe extern "C" fn(Handle, *mut [u8; 64]) -> i32,
    thermal: unsafe extern "C" fn(Handle, u32, *mut Thermal) -> i32,
}

// SAFETY: the function pointers are plain code addresses inside nvapi64.dll,
// which stays loaded for the life of the process.
unsafe impl Send for Api {}
// SAFETY: as above; NVAPI's DRS calls are thread-safe per session.
unsafe impl Sync for Api {}

// Each transmute's target type is the `Api` field it's assigned to, which
// spells out the function's signature.
#[allow(clippy::missing_transmute_annotations)]
fn load() -> Option<Api> {
    // SAFETY: loading the system NVIDIA library by name; it's never unloaded.
    let lib = unsafe { LoadLibraryW(&HSTRING::from("nvapi64.dll")) }.ok()?;
    // SAFETY: valid module handle and export name.
    let query = unsafe { GetProcAddress(lib, s!("nvapi_QueryInterface")) }?;
    // SAFETY: nvapi_QueryInterface has signature `void* (unsigned int id)`.
    let query: unsafe extern "C" fn(u32) -> *const c_void = unsafe { std::mem::transmute(query) };
    let get = |id: u32| -> Option<*const c_void> {
        // SAFETY: querying an interface ID; null means unsupported by this driver.
        let p = unsafe { query(id) };
        (!p.is_null()).then_some(p)
    };
    macro_rules! f {
        ($id:expr) => {
            // SAFETY: the ID's documented signature matches the field it's assigned to.
            unsafe { std::mem::transmute::<*const c_void, _>(get($id)?) }
        };
    }
    let initialize: unsafe extern "C" fn() -> i32 = f!(0x0150_E828);
    // SAFETY: NvAPI_Initialize takes no arguments and may be called more than once.
    if unsafe { initialize() } != NVAPI_OK {
        return None;
    }
    Some(Api {
        create_session: f!(0x0694_D52E),
        destroy_session: f!(0xDAD9_CFF8),
        load_settings: f!(0x375D_BD6B),
        save_settings: f!(0xFCBC_7E14),
        get_base_profile: f!(0xDA84_66A0),
        find_application: f!(0xEEE5_66B2),
        find_profile: f!(0x7E4A_9A0B),
        create_profile: f!(0xCC17_6068),
        delete_profile: f!(0x1709_3206),
        create_application: f!(0x4347_A9DE),
        get_setting: f!(0x73BF_8338),
        set_setting: f!(0x577D_D202),
        delete_setting: f!(0xE4A2_6362),
        save_to_file: f!(0x2BE2_5DF8),
        load_from_file: f!(0xD3ED_E889),
        driver_version: f!(0x2926_AAAD),
        enum_gpus: f!(0xE5AC_921F),
        gpu_name: f!(0xCEEE_8E9F),
        thermal: f!(0xE364_0A56),
    })
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(load).as_ref()
}

fn fail(op: &'static str, target: &str, code: i32) -> SysError {
    let kind = match code {
        NVAPI_SETTING_NOT_FOUND | NVAPI_PROFILE_NOT_FOUND | NVAPI_EXECUTABLE_NOT_FOUND => {
            SysErrorKind::NotFound
        }
        _ => SysErrorKind::Other,
    };
    SysError::new(kind, op, target, format!("NVAPI error {code}"))
}

fn check(op: &'static str, target: &str, code: i32) -> SysResult<()> {
    if code == NVAPI_OK {
        Ok(())
    } else {
        Err(fail(op, target, code))
    }
}

/// A loaded DRS session, destroyed on drop.
struct Session {
    api: &'static Api,
    handle: Handle,
}

impl Session {
    fn open(op: &'static str) -> SysResult<Self> {
        let api = api().ok_or_else(|| {
            SysError::new(
                SysErrorKind::NotFound,
                op,
                "NVIDIA",
                "No NVIDIA driver found.",
            )
        })?;
        let mut handle: Handle = std::ptr::null_mut();
        // SAFETY: valid out-pointer.
        check(op, "session", unsafe { (api.create_session)(&mut handle) })?;
        let session = Self { api, handle };
        // SAFETY: valid session handle.
        check(op, "session", unsafe {
            (api.load_settings)(session.handle)
        })?;
        Ok(session)
    }

    fn save(&self, op: &'static str) -> SysResult<()> {
        // SAFETY: valid session handle.
        check(op, "session", unsafe {
            (self.api.save_settings)(self.handle)
        })
    }

    /// `Ok(None)` when an app has no profile.
    fn profile(&self, profile: &NvProfile, op: &'static str) -> SysResult<Option<Handle>> {
        let mut h: Handle = std::ptr::null_mut();
        match profile {
            NvProfile::Global => {
                // SAFETY: valid session and out-pointer.
                check(op, "global profile", unsafe {
                    (self.api.get_base_profile)(self.handle, &mut h)
                })?;
                Ok(Some(h))
            }
            NvProfile::App { exe } => {
                let name = unicode(exe);
                let mut app = zeroed::<Application>();
                app.version = version::<Application>(4);
                // SAFETY: valid session, name buffer and out-pointers.
                match unsafe {
                    (self.api.find_application)(self.handle, name.as_ptr(), &mut h, &mut *app)
                } {
                    NVAPI_OK => Ok(Some(h)),
                    NVAPI_EXECUTABLE_NOT_FOUND | NVAPI_PROFILE_NOT_FOUND => Ok(None),
                    code => Err(fail(op, exe, code)),
                }
            }
        }
    }

    fn require(&self, profile: &NvProfile, op: &'static str) -> SysResult<Handle> {
        self.profile(profile, op)?.ok_or_else(|| {
            SysError::new(
                SysErrorKind::NotFound,
                op,
                format!("{profile:?}"),
                "The driver has no profile for this program.",
            )
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: handle created in `open`, destroyed once.
        unsafe { (self.api.destroy_session)(self.handle) };
    }
}

pub struct WinNvidia;

impl Nvidia for WinNvidia {
    fn info(&self) -> SysResult<Option<NvInfo>> {
        let Some(api) = api() else { return Ok(None) };
        let mut gpus = [std::ptr::null_mut(); 64];
        let mut count = 0u32;
        // SAFETY: valid out-pointers.
        if unsafe { (api.enum_gpus)(&mut gpus, &mut count) } != NVAPI_OK || count == 0 {
            return Ok(None);
        }
        let mut name = [0u8; 64];
        // SAFETY: valid GPU handle and buffer.
        unsafe { (api.gpu_name)(gpus[0], &mut name) };
        let mut driver = 0u32;
        let mut branch = [0u8; 64];
        // SAFETY: valid out-pointers.
        unsafe { (api.driver_version)(&mut driver, &mut branch) };
        let gpu = String::from_utf8_lossy(&name)
            .trim_end_matches('\0')
            .to_owned();
        Ok(Some(NvInfo {
            gpu: if gpu.starts_with("NVIDIA") {
                gpu
            } else {
                format!("NVIDIA {gpu}")
            },
            driver: format!("{}.{:02}", driver / 100, driver % 100),
        }))
    }

    fn get(&self, profile: &NvProfile, setting: u32) -> SysResult<Option<u32>> {
        const OP: &str = "read NVIDIA setting";
        let session = Session::open(OP)?;
        let Some(p) = session.profile(profile, OP)? else {
            return Ok(None);
        };
        let mut s = zeroed::<Setting>();
        s.version = version::<Setting>(1);
        // SAFETY: valid session, profile and setting buffer.
        match unsafe { (session.api.get_setting)(session.handle, p, setting, &mut *s) } {
            NVAPI_OK => {}
            NVAPI_SETTING_NOT_FOUND => return Ok(None),
            code => return Err(fail(OP, &format!("0x{setting:08X}"), code)),
        }
        // Only a value stored in this profile by the user counts; inherited
        // and predefined values mean "not set here".
        let (location, predefined, kind) = (s.location, s.is_current_predefined, s.kind);
        if location != CURRENT_PROFILE_LOCATION || predefined != 0 || kind != DWORD_TYPE {
            return Ok(None);
        }
        let current = s.current;
        Ok(Some(u32::from_le_bytes([
            current[0], current[1], current[2], current[3],
        ])))
    }

    fn set(&self, profile: &NvProfile, setting: u32, value: u32) -> SysResult<()> {
        const OP: &str = "change NVIDIA setting";
        let session = Session::open(OP)?;
        let p = session.require(profile, OP)?;
        let mut s = zeroed::<Setting>();
        s.version = version::<Setting>(1);
        s.id = setting;
        s.kind = DWORD_TYPE;
        let mut current = [0u8; 4100];
        current[..4].copy_from_slice(&value.to_le_bytes());
        s.current = current;
        // SAFETY: valid session, profile and setting.
        check(OP, &format!("0x{setting:08X}"), unsafe {
            (session.api.set_setting)(session.handle, p, &mut *s)
        })?;
        session.save(OP)
    }

    fn clear(&self, profile: &NvProfile, setting: u32) -> SysResult<()> {
        const OP: &str = "change NVIDIA setting";
        let session = Session::open(OP)?;
        let Some(p) = session.profile(profile, OP)? else {
            return Ok(());
        };
        // SAFETY: valid session and profile.
        match unsafe { (session.api.delete_setting)(session.handle, p, setting) } {
            NVAPI_OK => session.save(OP),
            NVAPI_SETTING_NOT_FOUND => Ok(()),
            code => Err(fail(OP, &format!("0x{setting:08X}"), code)),
        }
    }

    fn app_profile_exists(&self, exe: &str) -> SysResult<bool> {
        let session = Session::open("read NVIDIA profile")?;
        Ok(session
            .profile(&NvProfile::App { exe: exe.into() }, "read NVIDIA profile")?
            .is_some())
    }

    fn create_app_profile(&self, exe: &str) -> SysResult<()> {
        const OP: &str = "create NVIDIA profile";
        let session = Session::open(OP)?;
        let mut profile = zeroed::<Profile>();
        profile.version = version::<Profile>(1);
        profile.name = unicode(&format!("{PROFILE_PREFIX}{exe}"));
        let mut h: Handle = std::ptr::null_mut();
        // SAFETY: valid session, profile struct and out-pointer.
        check(OP, exe, unsafe {
            (session.api.create_profile)(session.handle, &mut *profile, &mut h)
        })?;
        let mut app = zeroed::<Application>();
        app.version = version::<Application>(4);
        app.app_name = unicode(exe);
        // SAFETY: valid session, new profile and application struct.
        check(OP, exe, unsafe {
            (session.api.create_application)(session.handle, h, &mut *app)
        })?;
        session.save(OP)
    }

    fn delete_app_profile(&self, exe: &str) -> SysResult<()> {
        const OP: &str = "delete NVIDIA profile";
        let session = Session::open(OP)?;
        let name = unicode(&format!("{PROFILE_PREFIX}{exe}"));
        let mut h: Handle = std::ptr::null_mut();
        // Looked up by Apex Tune's own profile name, so a driver profile is never deleted.
        // SAFETY: valid session, name buffer and out-pointer.
        match unsafe { (session.api.find_profile)(session.handle, name.as_ptr(), &mut h) } {
            NVAPI_OK => {}
            NVAPI_PROFILE_NOT_FOUND => return Ok(()),
            code => return Err(fail(OP, exe, code)),
        }
        // SAFETY: valid session and profile.
        check(OP, exe, unsafe {
            (session.api.delete_profile)(session.handle, h)
        })?;
        session.save(OP)
    }

    fn export(&self, path: &Path) -> SysResult<()> {
        const OP: &str = "back up NVIDIA settings";
        let session = Session::open(OP)?;
        let file = unicode(&path.display().to_string());
        // SAFETY: valid session and path buffer.
        check(OP, &path.display().to_string(), unsafe {
            (session.api.save_to_file)(session.handle, file.as_ptr())
        })
    }

    fn import(&self, path: &Path) -> SysResult<()> {
        const OP: &str = "restore NVIDIA settings";
        let session = Session::open(OP)?;
        let file = unicode(&path.display().to_string());
        // SAFETY: valid session and path buffer.
        check(OP, &path.display().to_string(), unsafe {
            (session.api.load_from_file)(session.handle, file.as_ptr())
        })?;
        session.save(OP)
    }

    fn gpu_temperature(&self) -> Option<i32> {
        let api = api()?;
        let mut gpus = [std::ptr::null_mut(); 64];
        let mut count = 0u32;
        // SAFETY: valid out-pointers.
        if unsafe { (api.enum_gpus)(&mut gpus, &mut count) } != NVAPI_OK || count == 0 {
            return None;
        }
        let mut thermal = Thermal {
            version: version::<Thermal>(2),
            ..Default::default()
        };
        // SAFETY: valid GPU handle and struct.
        if unsafe { (api.thermal)(gpus[0], THERMAL_TARGET_ALL, &mut thermal) } != NVAPI_OK {
            return None;
        }
        thermal
            .sensors
            .iter()
            .take(thermal.count as usize)
            .map(|s| s.current)
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_sizes_match_the_nvapi_headers() {
        assert_eq!(size_of::<Setting>(), 12320);
        assert_eq!(size_of::<Profile>(), 4116);
        assert_eq!(size_of::<Application>(), 20492);
        assert_eq!(size_of::<Thermal>(), 68);
    }
}
