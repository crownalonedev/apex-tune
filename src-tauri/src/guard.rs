//! Hard safety limits checked before every change, including reverts.
//!
//! Nothing here can be overridden from the UI. The rules cover anti-cheat
//! (Riot Vanguard, Easy Anti-Cheat, BattlEye), Windows security (Defender,
//! firewall, UAC, SmartScreen, Secure Boot / VBS, TPM) and Windows Update.

use std::path::Path;

use crate::sys::{Hive, RegData};
use crate::tweaks::atom::{Atom, RegTarget};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Refused to change {target}: {reason}")]
pub struct GuardViolation {
    pub target: String,
    pub reason: &'static str,
}

const ANTI_CHEAT: &str = "it belongs to an anti-cheat system";
const SECURITY: &str = "it controls a Windows security feature";
const UPDATES: &str = "it would turn off Windows Update";
const SYSTEM: &str = "it is a core Windows component";
const IFEO: &str = "only CPU priority under PerfOptions may be set in Image File Execution Options";
const AUTOSTART: &str = "Apex Tune never edits Run keys or Winlogon; startup items are disabled through StartupApproved";

/// Services that are never stopped, disabled or reconfigured. Matched
/// case-insensitively; entries ending in `*` are prefixes.
const PROTECTED_SERVICES: &[(&str, &str)] = &[
    ("vgc", ANTI_CHEAT),
    ("vgk", ANTI_CHEAT),
    ("easyanticheat*", ANTI_CHEAT),
    ("beservice*", ANTI_CHEAT),
    ("bedaisy", ANTI_CHEAT),
    ("windefend", SECURITY),
    ("wdnissvc", SECURITY),
    ("wdnisdrv", SECURITY),
    ("wdfilter", SECURITY),
    ("wdboot", SECURITY),
    ("sense", SECURITY),
    ("securityhealthservice", SECURITY),
    ("wscsvc", SECURITY),
    ("mpssvc", SECURITY),
    ("mpsdrv", SECURITY),
    ("bfe", SECURITY),
    ("appidsvc", SECURITY),
    ("tpm", SECURITY),
    ("wuauserv", UPDATES),
    ("usosvc", UPDATES),
    ("waasmedicsvc", UPDATES),
    ("bits", UPDATES),
    ("trustedinstaller", UPDATES),
    ("cryptsvc", SYSTEM),
    ("gamingservices*", SYSTEM),
    ("xblauthmanager", SYSTEM),
    ("xboxgipsvc", SYSTEM),
    ("nvdisplay.containerlocalsystem", SYSTEM),
];

/// Registry key prefixes (normalised, any hive) that are never written.
const PROTECTED_KEYS: &[(&str, &str)] = &[
    (r"software\policies\microsoft\windows defender", SECURITY),
    (r"software\microsoft\windows defender", SECURITY),
    (
        r"software\policies\microsoft\windows advanced threat protection",
        SECURITY,
    ),
    (r"software\policies\microsoft\windowsfirewall", SECURITY),
    (
        r"system\currentcontrolset\services\sharedaccess\parameters\firewallpolicy",
        SECURITY,
    ),
    (r"system\currentcontrolset\control\secureboot", SECURITY),
    (r"system\currentcontrolset\control\deviceguard", SECURITY),
    (r"system\currentcontrolset\control\lsa", SECURITY),
    (r"software\policies\microsoft\windows\deviceguard", SECURITY),
    (
        r"software\policies\microsoft\windows\windowsupdate\au",
        UPDATES,
    ),
    (
        r"software\microsoft\windows\currentversion\windowsupdate\auto update",
        UPDATES,
    ),
    (
        r"software\microsoft\windows nt\currentversion\silentprocessexit",
        SYSTEM,
    ),
    (
        r"software\microsoft\windows nt\currentversion\winlogon",
        AUTOSTART,
    ),
    (r"software\microsoft\windows\currentversion\run", AUTOSTART),
    (
        r"software\microsoft\windows\currentversion\runonce",
        AUTOSTART,
    ),
];

/// Values (key prefix, value name) that are never written.
const PROTECTED_VALUES: &[(&str, &str, &str)] = &[
    (
        r"software\microsoft\windows\currentversion\policies\system",
        "enablelua",
        SECURITY,
    ),
    (
        r"software\microsoft\windows\currentversion\policies\system",
        "consentpromptbehavioradmin",
        SECURITY,
    ),
    (
        r"software\microsoft\windows\currentversion\policies\system",
        "consentpromptbehavioruser",
        SECURITY,
    ),
    (
        r"software\microsoft\windows\currentversion\policies\system",
        "promptonsecuredesktop",
        SECURITY,
    ),
    (
        r"software\microsoft\windows\currentversion\policies\system",
        "filteradministratortoken",
        SECURITY,
    ),
    (
        r"software\microsoft\windows\currentversion\policies\system",
        "enableinstallerdetection",
        SECURITY,
    ),
    (
        r"software\microsoft\windows\currentversion\policies\system",
        "enablevirtualization",
        SECURITY,
    ),
    (
        r"software\policies\microsoft\windows\windowsupdate",
        "disablewindowsupdateaccess",
        UPDATES,
    ),
    (
        r"software\policies\microsoft\windows\windowsupdate",
        "setdisableuxwuaccess",
        UPDATES,
    ),
    (
        r"software\policies\microsoft\windows\windowsupdate",
        "donotconnecttowindowsupdateinternetlocations",
        UPDATES,
    ),
    (
        r"software\policies\microsoft\windows\windowsupdate",
        "wuserver",
        UPDATES,
    ),
    (
        r"software\policies\microsoft\windows\windowsupdate",
        "wustatusserver",
        UPDATES,
    ),
    (
        r"software\policies\microsoft\windows\windowsupdate",
        "usewuserver",
        UPDATES,
    ),
];

/// Words that mark a registry path, file path or exe as anti-cheat.
const ANTI_CHEAT_MARKERS: &[&str] = &[
    "vanguard",
    "easyanticheat",
    "battleye",
    "vgk.sys",
    "vgc.exe",
    "vgtray",
];

const IFEO_KEY: &str = r"software\microsoft\windows nt\currentversion\image file execution options";

/// Executables IFEO must never touch: anti-cheat launchers and core processes.
fn ifeo_exe_forbidden(exe: &str) -> Option<&'static str> {
    const CORE: &[&str] = &[
        "csrss.exe",
        "lsass.exe",
        "smss.exe",
        "wininit.exe",
        "winlogon.exe",
        "services.exe",
        "svchost.exe",
        "explorer.exe",
        "dwm.exe",
        "msmpeng.exe",
        "audiodg.exe",
        "fontdrvhost.exe",
    ];
    if is_anti_cheat_name(exe)
        || exe.contains("_eac")
        || exe.contains("eac_")
        || exe.ends_with("_be.exe")
    {
        Some(ANTI_CHEAT)
    } else if CORE.contains(&exe) {
        Some(SYSTEM)
    } else {
        None
    }
}

fn is_anti_cheat_name(s: &str) -> bool {
    ANTI_CHEAT_MARKERS.iter().any(|m| s.contains(m))
}

/// Lower-cases, uses single backslashes, maps `ControlSet00N` to
/// `CurrentControlSet` and drops `Wow6432Node`, so equivalent paths compare equal.
pub fn normalize_key(path: &str) -> String {
    let lower = path.replace('/', "\\").to_ascii_lowercase();
    let parts: Vec<&str> = lower
        .split('\\')
        .filter(|p| !p.is_empty() && *p != "wow6432node")
        .map(|p| {
            let is_numbered_set = p.len() == 13
                && p.starts_with("controlset")
                && p[10..].chars().all(|c| c.is_ascii_digit());
            if is_numbered_set {
                "currentcontrolset"
            } else {
                p
            }
        })
        .collect();
    parts.join("\\")
}

fn under(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('\\'))
}

fn service_protection(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    if name.contains("vanguard") {
        return Some(ANTI_CHEAT);
    }
    PROTECTED_SERVICES.iter().find_map(|(pattern, reason)| {
        let matches = match pattern.strip_suffix('*') {
            Some(prefix) => name.starts_with(prefix),
            None => name == *pattern,
        };
        matches.then_some(*reason)
    })
}

pub fn check_service(name: &str) -> Result<(), GuardViolation> {
    match service_protection(name) {
        Some(reason) => Err(GuardViolation {
            target: format!("service {name}"),
            reason,
        }),
        None => Ok(()),
    }
}

pub fn check_registry(
    hive: Hive,
    path: &str,
    name: &str,
    value: Option<&RegData>,
) -> Result<(), GuardViolation> {
    let key = normalize_key(path);
    let value_name = name.to_ascii_lowercase();
    let deny = |reason| {
        Err(GuardViolation {
            target: format!(r"{hive}\{path}\{name}"),
            reason,
        })
    };

    if is_anti_cheat_name(&key) {
        return deny(ANTI_CHEAT);
    }
    if key.contains("smartscreen") || value_name.contains("smartscreen") {
        return deny(SECURITY);
    }
    if key.contains(r"explorer\startupapproved") {
        if is_anti_cheat_name(&value_name) || value_name.contains("riot") {
            return deny(ANTI_CHEAT);
        }
        if value_name.contains("securityhealth") || value_name.contains("windows defender") {
            return deny(SECURITY);
        }
    }
    if let Some(service) = key.strip_prefix(r"system\currentcontrolset\services\") {
        let service = service.split('\\').next().unwrap_or_default();
        if let Some(reason) = service_protection(service) {
            return deny(reason);
        }
    }
    if let Some((_, reason)) = PROTECTED_KEYS
        .iter()
        .find(|(prefix, _)| under(&key, prefix))
    {
        return deny(reason);
    }
    if let Some((_, _, reason)) = PROTECTED_VALUES
        .iter()
        .find(|(prefix, v, _)| under(&key, prefix) && *v == value_name)
    {
        return deny(reason);
    }
    if under(&key, IFEO_KEY) {
        let rest: Vec<&str> = key[IFEO_KEY.len()..]
            .split('\\')
            .filter(|p| !p.is_empty())
            .collect();
        let allowed_shape =
            matches!(rest.as_slice(), [_, "perfoptions"]) && value_name == "cpupriorityclass";
        // Only High (3) or Above Normal (6) ever get written; a revert may delete the value.
        let allowed_value = matches!(value, None | Some(RegData::Dword(3 | 6)));
        if !allowed_shape || !allowed_value {
            return deny(IFEO);
        }
        if let Some(reason) = rest.first().and_then(|exe| ifeo_exe_forbidden(exe)) {
            return deny(reason);
        }
    }
    Ok(())
}

pub fn check_atom(atom: &Atom) -> Result<(), GuardViolation> {
    match atom {
        Atom::Registry {
            hive,
            path,
            name,
            target,
        } => {
            let value = match target {
                RegTarget::Set(data) => Some(data),
                RegTarget::Absent => None,
            };
            check_registry(*hive, path, name, value)
        }
        Atom::Service { name, .. } => check_service(name),
        Atom::PowerPlan { guid, name, .. } => check_power_plan(guid, name),
        Atom::PowerSetting { .. } | Atom::ActivePowerPlan { .. } => Ok(()),
        Atom::LogonTask {
            path,
            command,
            highest,
            ..
        } => check_own_task(path, command, *highest),
        Atom::TaskEnabled { path, .. } => check_task(path),
        Atom::AppRemoved { name, .. } => check_app(name),
    }
}

/// Only apps on Apex Tune's reviewed list can be removed; never the Store,
/// Xbox sign-in and Gaming Services that games need, or system apps.
fn check_app(name: &str) -> Result<(), GuardViolation> {
    if crate::debloat::apps::is_removable(name) {
        Ok(())
    } else {
        Err(GuardViolation {
            target: format!("app {name}"),
            reason: "it isn't on Apex Tune's list of apps that are safe to remove",
        })
    }
}

/// Task folders whose tasks keep Windows secure and updated.
const PROTECTED_TASK_FOLDERS: &[(&str, &str)] = &[
    (r"\microsoft\windows\windows defender", SECURITY),
    (r"\microsoft\windows\exploitguard", SECURITY),
    (r"\microsoft\windows\windowsupdate", UPDATES),
    (r"\microsoft\windows\updateorchestrator", UPDATES),
    (r"\microsoft\windows\waasmedic", UPDATES),
    (r"\microsoft\windows\tpm", SECURITY),
    (r"\microsoft\windows\bitlocker", SECURITY),
];

fn check_task(path: &str) -> Result<(), GuardViolation> {
    let lower = path.to_ascii_lowercase();
    let deny = |reason| {
        Err(GuardViolation {
            target: format!("task {path}"),
            reason,
        })
    };
    if is_anti_cheat_name(&lower) || lower.contains("riot") {
        return deny(ANTI_CHEAT);
    }
    if let Some((_, reason)) = PROTECTED_TASK_FOLDERS
        .iter()
        .find(|(f, _)| under(&lower, f))
    {
        return deny(reason);
    }
    Ok(())
}

/// Apex Tune only creates tasks in its own folder, and only Apex Tune itself runs elevated.
fn check_own_task(path: &str, command: &str, highest: bool) -> Result<(), GuardViolation> {
    let deny = |reason| {
        Err(GuardViolation {
            target: format!("task {path}"),
            reason,
        })
    };
    if !path.to_ascii_lowercase().starts_with(r"\apextune\") {
        return deny("Apex Tune only creates tasks in its own \\ApexTune folder");
    }
    let exe = command
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if highest && exe != "apex-tune.exe" {
        return deny("only Apex Tune itself may start with administrator rights");
    }
    Ok(())
}

/// Windows' own plans and templates. Apex Tune creates and deletes only its own.
const BUILT_IN_POWER_PLANS: &[&str] = &[
    "381b4222-f694-41f0-9685-ff5bb260df2e", // Balanced
    "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", // High performance
    "a1841308-3541-4fab-bc81-f71556f20b4a", // Power saver
    "e9a42b02-d5df-448d-aa00-03f14749eb61", // Ultimate Performance
];

fn check_power_plan(guid: &str, name: &str) -> Result<(), GuardViolation> {
    if BUILT_IN_POWER_PLANS
        .iter()
        .any(|b| b.eq_ignore_ascii_case(guid))
    {
        return Err(GuardViolation {
            target: format!("power plan {name}"),
            reason: "it is one of Windows' built-in plans",
        });
    }
    Ok(())
}

/// For file operations (cleaners): never inside anti-cheat folders.
pub fn check_path(path: &Path) -> Result<(), GuardViolation> {
    let lower = path
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");
    if lower
        .split('\\')
        .any(|part| part == "riot vanguard" || is_anti_cheat_name(part))
    {
        return Err(GuardViolation {
            target: path.display().to_string(),
            reason: ANTI_CHEAT,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(path: &str, name: &str) -> Result<(), GuardViolation> {
        check_registry(Hive::LocalMachine, path, name, Some(&RegData::Dword(3)))
    }

    #[test]
    fn normalizes_control_sets_and_wow64() {
        assert_eq!(
            normalize_key(r"SYSTEM\ControlSet001\Services\vgk\\"),
            r"system\currentcontrolset\services\vgk"
        );
        assert_eq!(normalize_key(r"SOFTWARE\WOW6432Node\Foo"), r"software\foo");
    }

    #[test]
    fn refuses_anti_cheat_services_and_keys() {
        for name in [
            "vgc",
            "VGK",
            "EasyAntiCheat_EOS",
            "BEService_x64",
            "BEDaisy",
        ] {
            assert!(check_service(name).is_err(), "{name} should be protected");
        }
        assert!(reg(r"SYSTEM\ControlSet001\Services\vgk", "Start").is_err());
        assert!(reg(r"SOFTWARE\Riot Vanguard", "x").is_err());
        assert!(check_service("Fax").is_ok());
        assert!(check_service("DiagTrack").is_ok());
    }

    #[test]
    fn refuses_security_and_update_kill_switches() {
        assert!(
            reg(
                r"SOFTWARE\Policies\Microsoft\Windows Defender",
                "DisableAntiSpyware"
            )
            .is_err()
        );
        assert!(
            reg(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System",
                "EnableLUA"
            )
            .is_err()
        );
        assert!(
            reg(
                r"SOFTWARE\Policies\Microsoft\Windows\System",
                "EnableSmartScreen"
            )
            .is_err()
        );
        assert!(
            reg(
                r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU",
                "NoAutoUpdate"
            )
            .is_err()
        );
        assert!(
            reg(
                r"SYSTEM\CurrentControlSet\Control\DeviceGuard",
                "EnableVirtualizationBasedSecurity"
            )
            .is_err()
        );
        assert!(check_service("wuauserv").is_err());
        assert!(check_service("WinDefend").is_err());
        // Deferring feature updates is allowed.
        assert!(
            reg(
                r"SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate",
                "DeferFeatureUpdates"
            )
            .is_ok()
        );
    }

    #[test]
    fn ifeo_allows_only_cpu_priority_for_ordinary_games() {
        let base = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options";
        assert!(
            reg(
                &format!(r"{base}\VALORANT-Win64-Shipping.exe\PerfOptions"),
                "CpuPriorityClass"
            )
            .is_ok()
        );
        assert!(reg(&format!(r"{base}\javaw.exe"), "Debugger").is_err());
        assert!(reg(&format!(r"{base}\javaw.exe\PerfOptions"), "IoPriority").is_err());
        assert!(reg(&format!(r"{base}\vgc.exe\PerfOptions"), "CpuPriorityClass").is_err());
        assert!(
            reg(
                &format!(r"{base}\FortniteClient-Win64-Shipping_EAC.exe\PerfOptions"),
                "CpuPriorityClass"
            )
            .is_err()
        );
        assert!(
            reg(
                &format!(r"{base}\lsass.exe\PerfOptions"),
                "CpuPriorityClass"
            )
            .is_err()
        );
        // Realtime (4) is never allowed.
        let realtime = check_registry(
            Hive::LocalMachine,
            &format!(r"{base}\javaw.exe\PerfOptions"),
            "CpuPriorityClass",
            Some(&RegData::Dword(4)),
        );
        assert!(realtime.is_err());
    }

    #[test]
    fn refuses_autostart_keys_but_allows_ordinary_tweaks() {
        assert!(
            check_registry(
                Hive::CurrentUser,
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                "x",
                None
            )
            .is_err()
        );
        assert!(
            check_registry(
                Hive::CurrentUser,
                r"Software\Microsoft\GameBar",
                "AutoGameModeEnabled",
                Some(&RegData::Dword(1))
            )
            .is_ok()
        );
        assert!(
            reg(
                r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers",
                "HwSchMode"
            )
            .is_ok()
        );
    }

    #[test]
    fn built_in_power_plans_are_never_managed_as_apex_plans() {
        let atom = |guid: &str| Atom::PowerPlan {
            guid: guid.into(),
            sources: vec![],
            name: "x".into(),
            description: String::new(),
        };
        assert!(check_atom(&atom("381B4222-F694-41F0-9685-FF5BB260DF2E")).is_err());
        assert!(check_atom(&atom("c855ae8b-fff9-45c7-8c29-99f6f893b54a")).is_ok());
    }

    #[test]
    fn tasks_are_limited_to_safe_targets() {
        let own = |path: &str, command: &str, highest| Atom::LogonTask {
            path: path.into(),
            command: command.into(),
            arguments: String::new(),
            description: String::new(),
            highest,
        };
        assert!(check_atom(&own(r"\ApexTune\Timer", r"C:\Apex\apex-timer.exe", false)).is_ok());
        assert!(check_atom(&own(r"\Microsoft\Evil", r"C:\Apex\apex-timer.exe", false)).is_err());
        assert!(check_atom(&own(r"\ApexTune\Timer", r"C:\Apex\apex-timer.exe", true)).is_err());
        assert!(check_atom(&own(r"\ApexTune\App", r"C:\Apex\apex-tune.exe", true)).is_ok());

        let toggle = |path: &str| Atom::TaskEnabled {
            path: path.into(),
            enabled: false,
        };
        assert!(check_atom(&toggle(r"\Microsoft\Windows\Maps\MapsUpdateTask")).is_ok());
        assert!(
            check_atom(&toggle(
                r"\Microsoft\Windows\Windows Defender\Windows Defender Scheduled Scan"
            ))
            .is_err()
        );
        assert!(
            check_atom(&toggle(
                r"\Microsoft\Windows\UpdateOrchestrator\Schedule Scan"
            ))
            .is_err()
        );
        assert!(check_atom(&toggle(r"\Riot Vanguard Tray")).is_err());
    }

    #[test]
    fn refuses_anti_cheat_folders() {
        assert!(check_path(Path::new(r"C:\Program Files\Riot Vanguard\vgc.exe")).is_err());
        assert!(check_path(Path::new(r"C:\Program Files (x86)\EasyAntiCheat_EOS\x")).is_err());
        assert!(check_path(Path::new(r"C:\Users\me\AppData\Local\Temp\foo.tmp")).is_ok());
    }
}
