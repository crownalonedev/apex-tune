//! System Restore through WMI (`ROOT\DEFAULT:SystemRestore`).

use serde::{Deserialize, Serialize};
use wmi::WMIConnection;

use crate::sys::{RestorePoint, RestorePoints, SysError, SysErrorKind, SysResult};

const NAMESPACE: &str = r"ROOT\DEFAULT";
/// `MODIFY_SETTINGS` and `BEGIN_SYSTEM_CHANGE` from SRRestorePtAPI.h.
const MODIFY_SETTINGS: u32 = 12;
const BEGIN_SYSTEM_CHANGE: u32 = 100;
/// SPP lists volumes with System Protection on under this client GUID.
const SPP_CLIENTS: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\SPP\Clients";
const SYSTEM_RESTORE_CLIENT: &str = "{09F7EDC5-294E-4180-AF6A-FB0E6A0E9513}";

#[derive(Deserialize)]
#[serde(rename = "SystemRestore")]
#[allow(non_snake_case)]
struct SystemRestore {
    Description: String,
    SequenceNumber: u32,
    CreationTime: String,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct CreateIn {
    Description: String,
    RestorePointType: u32,
    EventType: u32,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct EnableIn {
    Drive: String,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct MethodOut {
    ReturnValue: u32,
}

fn err(op: &'static str, message: impl Into<String>) -> SysError {
    SysError::new(SysErrorKind::Other, op, "System Restore", message)
}

fn connect(op: &'static str) -> SysResult<WMIConnection> {
    WMIConnection::with_namespace_path(NAMESPACE).map_err(|e| err(op, e.to_string()))
}

fn system_drive() -> String {
    std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
}

fn list_with(con: &WMIConnection) -> SysResult<Vec<RestorePoint>> {
    let rows: Vec<SystemRestore> = con
        .query()
        .map_err(|e| err("list restore points", e.to_string()))?;
    let mut points: Vec<RestorePoint> = rows
        .into_iter()
        .map(|r| RestorePoint {
            sequence: r.SequenceNumber,
            description: r.Description,
            created: r.CreationTime,
        })
        .collect();
    points.sort_by_key(|p| p.sequence);
    Ok(points)
}

pub struct WinRestorePoints;

impl RestorePoints for WinRestorePoints {
    fn protection_enabled(&self) -> SysResult<Option<bool>> {
        let Ok(key) = windows_registry::LOCAL_MACHINE.open(SPP_CLIENTS) else {
            return Ok(Some(false));
        };
        let volumes = match key.get_multi_string(SYSTEM_RESTORE_CLIENT) {
            Ok(v) => v,
            Err(e) if super::is_not_found(&e) => return Ok(Some(false)),
            Err(_) => return Ok(None),
        };
        // Entries look like `\\?\Volume{guid}\:(C%3A)`.
        let marker = format!("({}%3A)", system_drive().trim_end_matches(':')).to_ascii_lowercase();
        Ok(Some(
            volumes
                .iter()
                .any(|v| v.to_ascii_lowercase().contains(&marker)),
        ))
    }

    fn enable_protection(&self) -> SysResult<()> {
        let con = connect("enable System Protection")?;
        let out: MethodOut = con
            .exec_class_method::<SystemRestore, _>(
                "Enable",
                EnableIn {
                    Drive: format!(r"{}\", system_drive()),
                },
            )
            .map_err(|e| err("enable System Protection", e.to_string()))?;
        match out.ReturnValue {
            0 => Ok(()),
            code => Err(err(
                "enable System Protection",
                format!("Windows returned error {code}."),
            )),
        }
    }

    fn create(&self, description: &str) -> SysResult<RestorePoint> {
        let con = connect("create restore point")?;
        let before = list_with(&con)?.last().map_or(0, |p| p.sequence);
        let input = CreateIn {
            Description: description.to_owned(),
            RestorePointType: MODIFY_SETTINGS,
            EventType: BEGIN_SYSTEM_CHANGE,
        };
        let out: MethodOut = con
            .exec_class_method::<SystemRestore, _>("CreateRestorePoint", input)
            .map_err(|e| err("create restore point", e.to_string()))?;
        match out.ReturnValue {
            0 => {}
            1058 => {
                return Err(err(
                    "create restore point",
                    "System Protection is turned off.",
                ));
            }
            code => {
                return Err(err(
                    "create restore point",
                    format!("Windows returned error {code}."),
                ));
            }
        }
        // Windows reports success even when it skips a point, so check for a new one.
        list_with(&con)?
            .into_iter()
            .rfind(|p| p.sequence > before)
            .ok_or_else(|| {
                err(
                    "create restore point",
                    "Windows skipped it, usually because one was made recently.",
                )
            })
    }

    fn list(&self) -> SysResult<Vec<RestorePoint>> {
        list_with(&connect("list restore points")?)
    }
}
