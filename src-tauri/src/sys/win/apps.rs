//! App packages through Windows.Management.Deployment.PackageManager.

use windows::ApplicationModel::PackageSignatureKind;
use windows::Management::Deployment::{DeploymentOptions, DeploymentResult, PackageManager};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::HSTRING;

use super::sys_err;
use crate::sys::{AppPackage, Apps, SysError, SysErrorKind, SysResult};

fn manager(op: &'static str) -> SysResult<PackageManager> {
    // SAFETY: initialising COM on this thread; an already-initialised thread is fine.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    PackageManager::new().map_err(|e| sys_err(op, "PackageManager", &e))
}

fn check(
    op: &'static str,
    target: &str,
    result: windows::core::Result<DeploymentResult>,
) -> SysResult<()> {
    let result = result.map_err(|e| sys_err(op, target, &e))?;
    let code = result
        .ExtendedErrorCode()
        .map_err(|e| sys_err(op, target, &e))?;
    if code.is_ok() {
        return Ok(());
    }
    let text = result
        .ErrorText()
        .map(|t| t.to_string())
        .unwrap_or_default();
    Err(SysError::new(
        SysErrorKind::Other,
        op,
        target,
        if text.is_empty() {
            code.message()
        } else {
            text
        },
    ))
}

pub struct WinApps;

impl Apps for WinApps {
    fn list(&self) -> SysResult<Vec<AppPackage>> {
        const OP: &str = "list apps";
        let pm = manager(OP)?;
        // An empty SID means the current user.
        let packages = pm
            .FindPackagesByUserSecurityId(&HSTRING::new())
            .map_err(|e| sys_err(OP, "packages", &e))?;
        let mut out = Vec::new();
        for p in packages {
            let skip = p.IsFramework().unwrap_or(true) || p.IsResourcePackage().unwrap_or(true);
            if skip {
                continue;
            }
            let Ok(id) = p.Id() else { continue };
            let name = id.Name().map(|n| n.to_string()).unwrap_or_default();
            out.push(AppPackage {
                display_name: p
                    .DisplayName()
                    .map(|n| n.to_string())
                    .ok()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| name.clone()),
                family_name: id.FamilyName().map(|n| n.to_string()).unwrap_or_default(),
                full_name: id.FullName().map(|n| n.to_string()).unwrap_or_default(),
                system: p
                    .SignatureKind()
                    .is_ok_and(|k| k == PackageSignatureKind::System),
                name,
            });
        }
        Ok(out)
    }

    fn remove(&self, full_name: &str) -> SysResult<()> {
        const OP: &str = "remove app";
        let pm = manager(OP)?;
        let op = pm
            .RemovePackageAsync(&HSTRING::from(full_name))
            .map_err(|e| sys_err(OP, full_name, &e))?;
        check(OP, full_name, op.join())
    }

    fn restore(&self, full_name: &str) -> SysResult<()> {
        const OP: &str = "restore app";
        let pm = manager(OP)?;
        let op = pm
            .RegisterPackageByFullNameAsync(
                &HSTRING::from(full_name),
                None,
                DeploymentOptions::None,
            )
            .map_err(|e| sys_err(OP, full_name, &e))?;
        check(OP, full_name, op.join())
    }
}
