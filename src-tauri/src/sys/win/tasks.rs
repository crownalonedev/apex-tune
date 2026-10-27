//! Task Scheduler through its COM API (ITaskService).

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::TaskScheduler::{
    ITaskFolder, ITaskService, TASK_CREATE_OR_UPDATE, TASK_ENUM_HIDDEN,
    TASK_LOGON_INTERACTIVE_TOKEN, TaskScheduler,
};
use windows::Win32::System::Variant::VARIANT;
use windows::core::BSTR;

use super::{is_not_found, sys_err};
use crate::sys::task_xml::principal_user;
use crate::sys::{SysError, SysErrorKind, SysResult, TaskInfo, Tasks};

fn service(op: &'static str) -> SysResult<ITaskService> {
    // SAFETY: initialising COM on this thread; an already-initialised thread is fine.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // SAFETY: standard in-process COM activation.
    let svc: ITaskService = unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| sys_err(op, "Task Scheduler", &e))?;
    let empty = VARIANT::default();
    // SAFETY: empty variants mean the local machine and current user.
    unsafe { svc.Connect(&empty, &empty, &empty, &empty) }
        .map_err(|e| sys_err(op, "Task Scheduler", &e))?;
    Ok(svc)
}

/// `\A\B\Name` → (`\A\B`, `Name`).
fn split(path: &str) -> (String, String) {
    let trimmed = path.trim_end_matches('\\');
    match trimmed.rfind('\\') {
        Some(0) | None => (r"\".into(), trimmed.trim_start_matches('\\').into()),
        Some(i) => (trimmed[..i].into(), trimmed[i + 1..].into()),
    }
}

fn folder(svc: &ITaskService, path: &str, op: &'static str) -> SysResult<Option<ITaskFolder>> {
    // SAFETY: valid BSTR for the call.
    match unsafe { svc.GetFolder(&BSTR::from(path)) } {
        Ok(f) => Ok(Some(f)),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(sys_err(op, path, &e)),
    }
}

/// Opens a folder, creating it (one level under an existing parent) if missing.
fn folder_or_create(svc: &ITaskService, path: &str, op: &'static str) -> SysResult<ITaskFolder> {
    if let Some(f) = folder(svc, path, op)? {
        return Ok(f);
    }
    let (parent, name) = split(path);
    let parent = folder_or_create(svc, &parent, op)?;
    // SAFETY: valid BSTR; an empty variant means the default security descriptor.
    unsafe { parent.CreateFolder(&BSTR::from(name), &VARIANT::default()) }
        .map_err(|e| sys_err(op, path, &e))
}

pub struct WinTasks;

impl Tasks for WinTasks {
    fn get(&self, path: &str) -> SysResult<Option<(String, bool)>> {
        const OP: &str = "read scheduled task";
        let svc = service(OP)?;
        let (dir, name) = split(path);
        let Some(f) = folder(&svc, &dir, OP)? else {
            return Ok(None);
        };
        // SAFETY: valid BSTR.
        let task = match unsafe { f.GetTask(&BSTR::from(name)) } {
            Ok(t) => t,
            Err(e) if is_not_found(&e) => return Ok(None),
            Err(e) => return Err(sys_err(OP, path, &e)),
        };
        // SAFETY: `task` is a valid registered task.
        let xml = unsafe { task.Xml() }.map_err(|e| sys_err(OP, path, &e))?;
        // SAFETY: as above.
        let enabled = unsafe { task.Enabled() }.map_err(|e| sys_err(OP, path, &e))?;
        Ok(Some((xml.to_string(), enabled.as_bool())))
    }

    fn register(&self, path: &str, xml: &str) -> SysResult<()> {
        const OP: &str = "create scheduled task";
        let svc = service(OP)?;
        let (dir, name) = split(path);
        let f = folder_or_create(&svc, &dir, OP)?;
        let user = principal_user(xml)
            .map(|u| VARIANT::from(BSTR::from(u)))
            .unwrap_or_default();
        let empty = VARIANT::default();
        // SAFETY: valid BSTRs and variants for the call.
        unsafe {
            f.RegisterTask(
                &BSTR::from(name),
                &BSTR::from(xml),
                TASK_CREATE_OR_UPDATE.0,
                &user,
                &empty,
                TASK_LOGON_INTERACTIVE_TOKEN,
                &empty,
            )
        }
        .map(|_| ())
        .map_err(|e| sys_err(OP, path, &e))
    }

    fn delete(&self, path: &str) -> SysResult<()> {
        const OP: &str = "delete scheduled task";
        let svc = service(OP)?;
        let (dir, name) = split(path);
        let Some(f) = folder(&svc, &dir, OP)? else {
            return Ok(());
        };
        // SAFETY: valid BSTR.
        match unsafe { f.DeleteTask(&BSTR::from(name), 0) } {
            Ok(()) => Ok(()),
            Err(e) if is_not_found(&e) => Ok(()),
            Err(e) => Err(sys_err(OP, path, &e)),
        }
    }

    fn set_enabled(&self, path: &str, enabled: bool) -> SysResult<()> {
        const OP: &str = "change scheduled task";
        let svc = service(OP)?;
        let (dir, name) = split(path);
        let f = folder(&svc, &dir, OP)?.ok_or_else(|| {
            SysError::new(
                SysErrorKind::NotFound,
                OP,
                path,
                "The task folder doesn't exist.",
            )
        })?;
        // SAFETY: valid BSTR.
        let task = unsafe { f.GetTask(&BSTR::from(name)) }.map_err(|e| sys_err(OP, path, &e))?;
        // SAFETY: `task` is a valid registered task.
        unsafe { task.SetEnabled(enabled.into()) }.map_err(|e| sys_err(OP, path, &e))
    }

    fn list(&self, path: &str, recursive: bool) -> SysResult<Vec<TaskInfo>> {
        const OP: &str = "list scheduled tasks";
        let svc = service(OP)?;
        let Some(root) = folder(&svc, path, OP)? else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        collect(&root, recursive, &mut out).map_err(|e| sys_err(OP, path, &e))?;
        Ok(out)
    }

    fn interactive_account(&self) -> SysResult<String> {
        super::registry::interactive_account().ok_or_else(|| {
            SysError::new(
                SysErrorKind::NotFound,
                "read signed-in user",
                "console",
                "Nobody is signed in at the console.",
            )
        })
    }
}

fn collect(f: &ITaskFolder, recursive: bool, out: &mut Vec<TaskInfo>) -> windows::core::Result<()> {
    // SAFETY: valid folder; TASK_ENUM_HIDDEN includes hidden tasks.
    let tasks = unsafe { f.GetTasks(TASK_ENUM_HIDDEN.0) }?;
    // SAFETY: valid collection.
    let count = unsafe { tasks.Count() }?;
    for i in 1..=count {
        // SAFETY: collections are 1-based; `i` is within Count.
        let task = unsafe { tasks.get_Item(&VARIANT::from(i)) }?;
        // SAFETY: valid registered task.
        let (path, enabled) = unsafe { (task.Path()?, task.Enabled()?) };
        out.push(TaskInfo {
            path: path.to_string(),
            enabled: enabled.as_bool(),
        });
    }
    if recursive {
        // SAFETY: valid folder.
        let folders = unsafe { f.GetFolders(0) }?;
        // SAFETY: valid collection.
        let count = unsafe { folders.Count() }?;
        for i in 1..=count {
            // SAFETY: 1-based index within Count.
            let child = unsafe { folders.get_Item(&VARIANT::from(i)) }?;
            collect(&child, true, out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::split;

    #[test]
    fn splits_paths() {
        assert_eq!(
            split(r"\ApexTune\Timer"),
            (r"\ApexTune".into(), "Timer".into())
        );
        assert_eq!(split(r"\Root"), (r"\".into(), "Root".into()));
    }
}
