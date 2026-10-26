//! WMI queries for GPUs, physical drives and the battery.

use serde::{Deserialize, Deserializer};
use wmi::WMIConnection;

use super::gpu::{GpuInfo, GpuVendor, nvidia_driver_version, vendor_from, vram_from_registry};
use super::{DriveInfo, drive_kind};
use crate::sys::SysCtx;

pub struct Probe {
    pub gpus: Vec<GpuInfo>,
    pub drives: Vec<DriveInfo>,
    pub has_battery: bool,
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_VideoController")]
#[allow(non_snake_case)]
struct VideoController {
    Name: Option<String>,
    AdapterCompatibility: Option<String>,
    DriverVersion: Option<String>,
    PNPDeviceID: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename = "MSFT_PhysicalDisk")]
#[allow(non_snake_case)]
struct PhysicalDisk {
    FriendlyName: Option<String>,
    BusType: Option<u16>,
    MediaType: Option<u16>,
    /// WMI returns uint64 as a string.
    #[serde(deserialize_with = "u64_or_string")]
    Size: u64,
}

#[derive(Deserialize)]
#[serde(rename = "Win32_Battery")]
#[allow(non_snake_case)]
struct Battery {
    #[allow(dead_code)]
    DeviceID: Option<String>,
}

fn u64_or_string<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        N(u64),
        S(String),
    }
    Ok(match Option::<Raw>::deserialize(d)? {
        Some(Raw::N(n)) => n,
        Some(Raw::S(s)) => s.trim().parse().unwrap_or(0),
        None => 0,
    })
}

pub fn probe(ctx: &SysCtx) -> Probe {
    let mut warnings = Vec::new();
    let mut gpus = Vec::new();
    let mut drives = Vec::new();
    let mut has_battery = false;

    match WMIConnection::new() {
        Ok(con) => {
            match con.query::<VideoController>() {
                Ok(rows) => {
                    gpus = rows
                        .into_iter()
                        // Virtual adapters (remote desktop, streaming tools) enumerate under ROOT\.
                        .filter(|r| {
                            !r.PNPDeviceID
                                .as_deref()
                                .unwrap_or("")
                                .to_ascii_uppercase()
                                .starts_with(r"ROOT\")
                        })
                        .map(|r| {
                            let name = r.Name.unwrap_or_else(|| "Unknown GPU".into());
                            let vendor =
                                vendor_from(r.AdapterCompatibility.as_deref().unwrap_or(""), &name);
                            let driver_version = r.DriverVersion.unwrap_or_default();
                            GpuInfo {
                                nvidia_driver: (vendor == GpuVendor::Nvidia)
                                    .then(|| nvidia_driver_version(&driver_version))
                                    .flatten(),
                                vram_bytes: vram_from_registry(ctx, &name),
                                name,
                                vendor,
                                driver_version,
                            }
                        })
                        .collect();
                }
                Err(e) => warnings.push(format!("Graphics cards: {e}")),
            }
            match con.query::<Battery>() {
                Ok(rows) => has_battery = !rows.is_empty(),
                Err(e) => warnings.push(format!("Battery: {e}")),
            }
        }
        Err(e) => warnings.push(format!("WMI unavailable: {e}")),
    }

    match WMIConnection::with_namespace_path(r"ROOT\Microsoft\Windows\Storage")
        .and_then(|c| c.query::<PhysicalDisk>())
    {
        Ok(rows) => {
            drives = rows
                .into_iter()
                .map(|r| DriveInfo {
                    name: r.FriendlyName.unwrap_or_else(|| "Unknown drive".into()),
                    kind: drive_kind(r.BusType.unwrap_or(0), r.MediaType.unwrap_or(0)),
                    size_bytes: r.Size,
                })
                .collect();
        }
        Err(e) => warnings.push(format!("Drives: {e}")),
    }

    Probe {
        gpus,
        drives,
        has_battery,
        warnings,
    }
}
