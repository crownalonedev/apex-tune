//! Startup apps: Run keys, Startup folders and third-party sign-in tasks.
//!
//! Run-key and folder entries are switched off the way Task Manager does it,
//! through `StartupApproved`, so the entry itself is never deleted.

use std::path::PathBuf;

use serde::Serialize;

use crate::sys::{Hive, RegData, SysCtx, SysResult};
use crate::tweaks::atom::Atom;

const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// HKCU Run
    UserRun,
    /// HKLM Run
    MachineRun,
    /// HKLM WOW6432Node Run (32-bit apps)
    MachineRun32,
    UserFolder,
    CommonFolder,
    Task,
}

impl Source {
    /// Where Task Manager records the on/off state for this kind of entry.
    fn approved(self) -> Option<(Hive, String)> {
        let (hive, sub) = match self {
            Source::UserRun => (Hive::CurrentUser, "Run"),
            Source::MachineRun => (Hive::LocalMachine, "Run"),
            Source::MachineRun32 => (Hive::LocalMachine, "Run32"),
            Source::UserFolder => (Hive::CurrentUser, "StartupFolder"),
            Source::CommonFolder => (Hive::LocalMachine, "StartupFolder"),
            Source::Task => return None,
        };
        Some((hive, format!(r"{APPROVED}\{sub}")))
    }

    fn key(self) -> &'static str {
        match self {
            Source::UserRun => "userRun",
            Source::MachineRun => "machineRun",
            Source::MachineRun32 => "machineRun32",
            Source::UserFolder => "userFolder",
            Source::CommonFolder => "commonFolder",
            Source::Task => "task",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    /// `source:name`, stable across runs.
    pub id: String,
    pub name: String,
    pub command: String,
    pub source: Source,
    pub enabled: bool,
    /// Why it can't be switched off here, if it can't.
    pub protected: Option<String>,
}

/// Task Manager's "disabled" marker: first byte odd, then a timestamp it doesn't need.
const DISABLED: [u8; 12] = [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn approved_enabled(ctx: &SysCtx, source: Source, name: &str) -> SysResult<bool> {
    let Some((hive, key)) = source.approved() else {
        return Ok(true);
    };
    Ok(match ctx.registry.get(hive, &key, name)? {
        Some(RegData::Binary(b)) => b.first().is_none_or(|first| first % 2 == 0),
        _ => true,
    })
}

fn protection(name: &str, command: &str) -> Option<String> {
    let s = format!("{name} {command}").to_ascii_lowercase();
    if ["vanguard", "vgtray", "riot", "easyanticheat", "battleye"]
        .iter()
        .any(|m| s.contains(m))
    {
        Some("Anti-cheat; games need it running.".into())
    } else if s.contains("securityhealth") || s.contains("windows defender") {
        Some("Windows Security.".into())
    } else if s.contains("apex-tune") || s.contains("apex-timer") {
        Some("Managed on Apex Tune's own pages.".into())
    } else {
        None
    }
}

fn reg_string(d: &RegData) -> String {
    match d {
        RegData::Sz(s) | RegData::ExpandSz(s) => s.clone(),
        other => other.to_string(),
    }
}

fn push(
    items: &mut Vec<StartupItem>,
    ctx: &SysCtx,
    source: Source,
    name: String,
    command: String,
) -> SysResult<()> {
    let enabled = approved_enabled(ctx, source, &name)?;
    items.push(StartupItem {
        id: format!("{}:{name}", source.key()),
        protected: protection(&name, &command),
        name,
        command,
        source,
        enabled,
    });
    Ok(())
}

fn folder_entries(dir: Option<PathBuf>) -> Vec<String> {
    let Some(dir) = dir else { return Vec::new() };
    std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.eq_ignore_ascii_case("desktop.ini"))
                .collect()
        })
        .unwrap_or_default()
}

fn user_startup_folder() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\Startup"))
}

fn common_startup_folder() -> Option<PathBuf> {
    std::env::var_os("ProgramData")
        .map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\StartUp"))
}

/// Third-party tasks that run at sign-in or boot (Microsoft's own are left out).
fn logon_tasks(ctx: &SysCtx) -> SysResult<Vec<(String, String, bool)>> {
    let mut out = Vec::new();
    for task in ctx.tasks.list(r"\", true)? {
        let lower = task.path.to_ascii_lowercase();
        if lower.starts_with(r"\microsoft\") || lower.starts_with(r"\apextune\") {
            continue;
        }
        let Some((xml, _)) = ctx.tasks.get(&task.path)? else {
            continue;
        };
        if !(xml.contains("<LogonTrigger") || xml.contains("<BootTrigger")) {
            continue;
        }
        let command = crate::sys::task_xml::exec_action(&xml)
            .map(|(c, a)| format!("{c} {a}").trim().to_owned())
            .unwrap_or_default();
        out.push((task.path, command, task.enabled));
    }
    Ok(out)
}

pub fn list(ctx: &SysCtx) -> SysResult<Vec<StartupItem>> {
    let mut items = Vec::new();
    for (source, hive, key) in [
        (Source::UserRun, Hive::CurrentUser, RUN),
        (Source::MachineRun, Hive::LocalMachine, RUN),
        (Source::MachineRun32, Hive::LocalMachine, RUN32),
    ] {
        for (name, data) in ctx.registry.values(hive, key)? {
            push(&mut items, ctx, source, name, reg_string(&data))?;
        }
    }
    for (source, dir) in [
        (Source::UserFolder, user_startup_folder()),
        (Source::CommonFolder, common_startup_folder()),
    ] {
        for name in folder_entries(dir) {
            push(&mut items, ctx, source, name.clone(), name)?;
        }
    }
    for (path, command, enabled) in logon_tasks(ctx)? {
        items.push(StartupItem {
            id: format!("task:{path}"),
            name: path.rsplit('\\').next().unwrap_or(&path).to_owned(),
            protected: protection(&path, &command),
            command,
            source: Source::Task,
            enabled,
        });
    }
    items.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    Ok(items)
}

/// The atom that switches one item on or off.
pub fn toggle(item: &StartupItem, enabled: bool) -> Atom {
    match item.source.approved() {
        Some((hive, key)) => Atom::reg(
            hive,
            &key,
            &item.name,
            RegData::Binary(if enabled { ENABLED } else { DISABLED }.to_vec()),
        ),
        None => Atom::TaskEnabled {
            path: item.id.trim_start_matches("task:").to_owned(),
            enabled,
        },
    }
}

pub fn action_id(item_id: &str) -> String {
    format!("startup.{item_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::Engine;

    #[test]
    fn lists_run_entries_and_task_manager_state() {
        let sys = FakeSystem::new();
        sys.registry.seed(
            Hive::CurrentUser,
            RUN,
            "Discord",
            RegData::Sz(r"C:\Discord\Update.exe --processStart".into()),
        );
        sys.registry.seed(
            Hive::LocalMachine,
            RUN,
            "SecurityHealth",
            RegData::ExpandSz("SecurityHealthSystray.exe".into()),
        );
        sys.registry.seed(
            Hive::CurrentUser,
            &format!(r"{APPROVED}\Run"),
            "Discord",
            RegData::Binary(DISABLED.to_vec()),
        );
        let items = list(&sys.ctx).unwrap();
        let discord = items
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case("discord"))
            .unwrap();
        assert!(!discord.enabled);
        let security = items
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case("securityhealth"))
            .unwrap();
        assert!(security.protected.is_some());
    }

    #[test]
    fn disabling_is_reversible_and_never_deletes_the_entry() {
        let sys = FakeSystem::new();
        sys.registry.seed(
            Hive::CurrentUser,
            RUN,
            "Steam",
            RegData::Sz("steam.exe -silent".into()),
        );
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        let item = list(&sys.ctx).unwrap().into_iter().next().unwrap();

        engine
            .apply_action(
                &action_id(&item.id),
                "Startup off: Steam",
                vec![toggle(&item, false)],
            )
            .unwrap();
        assert!(!list(&sys.ctx).unwrap()[0].enabled);
        assert!(
            sys.registry
                .value(Hive::CurrentUser, RUN, "Steam")
                .is_some(),
            "Run entry kept"
        );

        engine.revert_action(&action_id(&item.id)).unwrap();
        assert!(list(&sys.ctx).unwrap()[0].enabled);
    }

    #[test]
    fn third_party_logon_tasks_are_listed_and_microsoft_ones_skipped() {
        let sys = FakeSystem::new();
        let xml = r#"<Task><Triggers><LogonTrigger/></Triggers><Actions><Exec><Command>C:\x\updater.exe</Command></Exec></Actions></Task>"#;
        sys.ctx.tasks.register(r"\Vendor\Updater", xml).unwrap();
        sys.ctx
            .tasks
            .register(r"\Microsoft\Windows\Something", xml)
            .unwrap();
        let items = list(&sys.ctx).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, Source::Task);
        assert_eq!(items[0].command, r"C:\x\updater.exe");
    }

    #[test]
    fn guard_refuses_switching_off_security_or_anti_cheat() {
        let atom = Atom::reg(
            Hive::LocalMachine,
            &format!(r"{APPROVED}\Run"),
            "SecurityHealth",
            RegData::Binary(DISABLED.to_vec()),
        );
        assert!(crate::guard::check_atom(&atom).is_err());
        let atom = Atom::reg(
            Hive::LocalMachine,
            &format!(r"{APPROVED}\Run"),
            "Riot Vanguard",
            RegData::Binary(DISABLED.to_vec()),
        );
        assert!(crate::guard::check_atom(&atom).is_err());
    }
}
