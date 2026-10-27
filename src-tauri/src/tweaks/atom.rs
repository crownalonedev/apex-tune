//! Atoms: the smallest reversible changes. A declarative tweak is a list of
//! atoms; each atom can capture what's there now, write its target, restore
//! the captured original, and say whether its target is in place.

use serde::{Deserialize, Serialize};

use crate::sys::task_xml::{self, LogonTask};
use crate::sys::{
    Hive, NvProfile, PowerSource, RegData, ServiceState, SpiFlag, StartType, SysCtx, SysError,
    SysErrorKind, SysResult,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "data", rename_all = "camelCase")]
pub enum RegTarget {
    Set(RegData),
    /// The value must not exist (e.g. removing a policy).
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Atom {
    Registry {
        hive: Hive,
        path: String,
        name: String,
        target: RegTarget,
    },
    Service {
        name: String,
        start: StartType,
        /// Also stop it now if it's running.
        stop: bool,
    },
    /// A power plan with a fixed GUID, copied from the first source that works.
    PowerPlan {
        guid: String,
        sources: Vec<String>,
        name: String,
        description: String,
    },
    PowerSetting {
        scheme: String,
        subgroup: String,
        setting: String,
        /// Human name, e.g. "Minimum processor state".
        label: String,
        unit: PowerUnit,
        source: PowerSource,
        value: u32,
    },
    ActivePowerPlan {
        guid: String,
    },
    /// One of Apex Tune's own tasks that starts something at sign-in.
    LogonTask {
        /// Under `\ApexTune\`.
        path: String,
        command: String,
        arguments: String,
        description: String,
        /// Run with highest privileges (only Apex Tune itself needs this).
        highest: bool,
    },
    /// Enables or disables an existing scheduled task.
    TaskEnabled {
        path: String,
        enabled: bool,
    },
    /// Removes a preinstalled app for the current user.
    AppRemoved {
        name: String,
        full_name: String,
        display_name: String,
    },
    /// A desktop setting applied through SystemParametersInfo.
    Spi {
        flag: SpiFlag,
        value: bool,
    },
    DisplayRefresh {
        device: String,
        monitor: String,
        hz: u32,
    },
    /// DNS servers for one adapter; empty lists mean automatic.
    Dns {
        adapter: String,
        adapter_name: String,
        v4: Vec<String>,
        v6: Vec<String>,
    },
    /// An NVIDIA driver profile for a program that had none.
    NvAppProfile {
        exe: String,
    },
    /// One NVIDIA driver setting in a profile.
    NvSetting {
        profile: NvProfile,
        setting: u32,
        label: String,
        value: u32,
        value_label: String,
    },
}

/// How a power setting's raw value reads to a person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "choices", rename_all = "camelCase")]
pub enum PowerUnit {
    Percent,
    /// 0 means "Never".
    Seconds,
    /// Index into named choices.
    Choice(Vec<String>),
}

impl PowerUnit {
    pub fn format(&self, value: u32) -> String {
        match self {
            PowerUnit::Percent => format!("{value}%"),
            PowerUnit::Seconds if value == 0 => "Never".into(),
            PowerUnit::Seconds if value.is_multiple_of(3600) => format!("{} h", value / 3600),
            PowerUnit::Seconds if value.is_multiple_of(60) => format!("{} min", value / 60),
            PowerUnit::Seconds => format!("{value} s"),
            PowerUnit::Choice(names) => names
                .get(value as usize)
                .cloned()
                .unwrap_or_else(|| value.to_string()),
        }
    }
}

/// What was there before an atom was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Original {
    Registry {
        value: Option<RegData>,
        /// Keys this change had to create, deepest first. Removed on revert if empty.
        created_keys: Vec<String>,
    },
    Service {
        /// `None` when the service isn't installed; the atom then does nothing.
        state: Option<ServiceState>,
    },
    PowerPlan {
        existed: bool,
    },
    PowerSetting {
        /// `None` when the plan or setting didn't exist.
        value: Option<u32>,
    },
    ActivePowerPlan {
        guid: String,
    },
    LogonTask {
        /// The previous definition, if the task existed.
        xml: Option<String>,
        enabled: bool,
    },
    TaskEnabled {
        /// `None` when the task doesn't exist; the atom then does nothing.
        enabled: Option<bool>,
    },
    AppRemoved {
        installed: bool,
    },
    Spi {
        value: bool,
    },
    DisplayRefresh {
        hz: u32,
    },
    Dns {
        v4: Vec<String>,
        v6: Vec<String>,
    },
    NvAppProfile {
        existed: bool,
    },
    NvSetting {
        /// `None` means the profile inherited the default.
        value: Option<u32>,
    },
}

/// One line of a change preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePreview {
    pub target: String,
    pub from: String,
    pub to: String,
}

const ABSENT: &str = "(not set)";

impl Atom {
    pub fn reg(hive: Hive, path: &str, name: &str, data: RegData) -> Self {
        Atom::Registry {
            hive,
            path: path.into(),
            name: name.into(),
            target: RegTarget::Set(data),
        }
    }

    pub fn reg_dword(hive: Hive, path: &str, name: &str, value: u32) -> Self {
        Self::reg(hive, path, name, RegData::Dword(value))
    }

    pub fn reg_sz(hive: Hive, path: &str, name: &str, value: &str) -> Self {
        Self::reg(hive, path, name, RegData::Sz(value.into()))
    }

    pub fn reg_absent(hive: Hive, path: &str, name: &str) -> Self {
        Atom::Registry {
            hive,
            path: path.into(),
            name: name.into(),
            target: RegTarget::Absent,
        }
    }

    pub fn service(name: &str, start: StartType, stop: bool) -> Self {
        Atom::Service {
            name: name.into(),
            start,
            stop,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Atom::Registry {
                hive, path, name, ..
            } => format!(r"{hive}\{path}\{name}"),
            Atom::Service { name, .. } => format!("Service {name}"),
            Atom::PowerPlan { name, .. } => format!("Power plan \"{name}\""),
            Atom::PowerSetting { label, source, .. } => match source {
                PowerSource::Ac => format!("{label} (plugged in)"),
                PowerSource::Dc => format!("{label} (on battery)"),
            },
            Atom::ActivePowerPlan { .. } => "Active power plan".into(),
            Atom::LogonTask { path, .. } => format!("Sign-in task {path}"),
            Atom::TaskEnabled { path, .. } => format!("Scheduled task {path}"),
            Atom::AppRemoved { display_name, .. } => format!("App {display_name}"),
            Atom::Spi { flag, .. } => flag.label().into(),
            Atom::DisplayRefresh { monitor, .. } => format!("Refresh rate of {monitor}"),
            Atom::Dns { adapter_name, .. } => format!("DNS servers for {adapter_name}"),
            Atom::NvAppProfile { exe } => format!("NVIDIA profile for {exe}"),
            Atom::NvSetting { profile, label, .. } => match profile {
                NvProfile::Global => format!("NVIDIA global: {label}"),
                NvProfile::App { exe } => format!("NVIDIA {exe}: {label}"),
            },
        }
    }

    fn app_installed(ctx: &SysCtx, name: &str) -> SysResult<bool> {
        Ok(ctx
            .apps
            .list()?
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(name)))
    }

    /// A setting's own value, treating "the app has no profile yet" as unset.
    fn nv_get(ctx: &SysCtx, profile: &NvProfile, setting: u32) -> SysResult<Option<u32>> {
        if let NvProfile::App { exe } = profile
            && !ctx.nvidia.app_profile_exists(exe)?
        {
            return Ok(None);
        }
        ctx.nvidia.get(profile, setting)
    }

    fn current_hz(ctx: &SysCtx, device: &str) -> SysResult<Option<u32>> {
        Ok(ctx
            .desktop
            .displays()?
            .into_iter()
            .find(|d| d.device == device)
            .map(|d| d.hz))
    }

    /// DNS servers as Windows stores them: a static list, or empty for automatic.
    fn current_dns(ctx: &SysCtx, adapter: &str) -> SysResult<(Vec<String>, Vec<String>)> {
        let read = |stack: &str| -> SysResult<Vec<String>> {
            let path = format!(
                r"SYSTEM\CurrentControlSet\Services\{stack}\Parameters\Interfaces\{adapter}"
            );
            Ok(
                match ctx.registry.get(Hive::LocalMachine, &path, "NameServer")? {
                    Some(RegData::Sz(s)) => s
                        .split([',', ' '])
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    _ => Vec::new(),
                },
            )
        };
        Ok((read("Tcpip")?, read("Tcpip6")?))
    }

    fn logon_xml(
        ctx: &SysCtx,
        command: &str,
        arguments: &str,
        description: &str,
        highest: bool,
    ) -> SysResult<String> {
        let account = ctx.tasks.interactive_account()?;
        Ok(LogonTask {
            account: &account,
            command,
            arguments,
            description,
            highest,
        }
        .to_xml())
    }

    pub fn capture(&self, ctx: &SysCtx) -> SysResult<Original> {
        match self {
            Atom::Registry {
                hive, path, name, ..
            } => Ok(Original::Registry {
                value: ctx.registry.get(*hive, path, name)?,
                created_keys: missing_keys(ctx, *hive, path)?,
            }),
            Atom::Service { name, .. } => Ok(Original::Service {
                state: ctx.services.query(name)?,
            }),
            Atom::PowerPlan { guid, .. } => Ok(Original::PowerPlan {
                existed: ctx.power.exists(guid)?,
            }),
            Atom::PowerSetting {
                scheme,
                subgroup,
                setting,
                source,
                ..
            } => Ok(Original::PowerSetting {
                value: ctx.power.read(scheme, subgroup, setting, *source)?,
            }),
            Atom::ActivePowerPlan { .. } => Ok(Original::ActivePowerPlan {
                guid: ctx.power.active()?,
            }),
            Atom::LogonTask { path, .. } => Ok(match ctx.tasks.get(path)? {
                Some((xml, enabled)) => Original::LogonTask {
                    xml: Some(xml),
                    enabled,
                },
                None => Original::LogonTask {
                    xml: None,
                    enabled: false,
                },
            }),
            Atom::TaskEnabled { path, .. } => Ok(Original::TaskEnabled {
                enabled: ctx.tasks.get(path)?.map(|(_, enabled)| enabled),
            }),
            Atom::AppRemoved { name, .. } => Ok(Original::AppRemoved {
                installed: Self::app_installed(ctx, name)?,
            }),
            Atom::Spi { flag, .. } => Ok(Original::Spi {
                value: ctx.desktop.spi_get(*flag)?,
            }),
            Atom::DisplayRefresh {
                device, monitor, ..
            } => Ok(Original::DisplayRefresh {
                hz: Self::current_hz(ctx, device)?.ok_or_else(|| {
                    SysError::new(
                        SysErrorKind::NotFound,
                        "read refresh rate",
                        monitor,
                        "Display not found.",
                    )
                })?,
            }),
            Atom::Dns { adapter, .. } => {
                let (v4, v6) = Self::current_dns(ctx, adapter)?;
                Ok(Original::Dns { v4, v6 })
            }
            Atom::NvAppProfile { exe } => Ok(Original::NvAppProfile {
                existed: ctx.nvidia.app_profile_exists(exe)?,
            }),
            Atom::NvSetting {
                profile, setting, ..
            } => Ok(Original::NvSetting {
                value: Self::nv_get(ctx, profile, *setting)?,
            }),
        }
    }

    pub fn write(&self, ctx: &SysCtx) -> SysResult<()> {
        match self {
            Atom::Registry {
                hive,
                path,
                name,
                target,
            } => match target {
                RegTarget::Set(data) => ctx.registry.set(*hive, path, name, data),
                RegTarget::Absent => ctx.registry.delete_value(*hive, path, name),
            },
            Atom::Service { name, start, stop } => {
                let Some(state) = ctx.services.query(name)? else {
                    return Ok(());
                };
                if state.start != *start {
                    ctx.services.set_start(name, *start)?;
                }
                if *stop && state.running {
                    ctx.services.stop(name)?;
                }
                Ok(())
            }
            Atom::PowerPlan {
                guid,
                sources,
                name,
                description,
            } => {
                if ctx.power.exists(guid)? {
                    return Ok(());
                }
                let mut last = None;
                for source in sources {
                    match ctx.power.duplicate(source, guid) {
                        Ok(()) => return ctx.power.rename(guid, name, description),
                        Err(e) => last = Some(e),
                    }
                }
                Err(last.unwrap_or_else(|| {
                    SysError::new(
                        SysErrorKind::Other,
                        "create power plan",
                        name,
                        "no source plan given",
                    )
                }))
            }
            Atom::PowerSetting {
                scheme,
                subgroup,
                setting,
                source,
                value,
                ..
            } => ctx.power.write(scheme, subgroup, setting, *source, *value),
            Atom::ActivePowerPlan { guid } => {
                if ctx.power.active()?.eq_ignore_ascii_case(guid) {
                    return Ok(());
                }
                ctx.power.set_active(guid)
            }
            Atom::LogonTask {
                path,
                command,
                arguments,
                description,
                highest,
            } => ctx.tasks.register(
                path,
                &Self::logon_xml(ctx, command, arguments, description, *highest)?,
            ),
            Atom::TaskEnabled { path, enabled } => match ctx.tasks.get(path)? {
                Some((_, now)) if now != *enabled => ctx.tasks.set_enabled(path, *enabled),
                _ => Ok(()),
            },
            Atom::AppRemoved {
                name, full_name, ..
            } => {
                if Self::app_installed(ctx, name)? {
                    ctx.apps.remove(full_name)?;
                }
                Ok(())
            }
            Atom::Spi { flag, value } => {
                if ctx.desktop.spi_get(*flag)? == *value {
                    return Ok(());
                }
                ctx.desktop.spi_set(*flag, *value)
            }
            Atom::DisplayRefresh { device, hz, .. } => {
                if Self::current_hz(ctx, device)? == Some(*hz) {
                    return Ok(());
                }
                ctx.desktop.set_refresh(device, *hz)
            }
            Atom::Dns {
                adapter, v4, v6, ..
            } => ctx.network.set_dns(adapter, v4, v6),
            Atom::NvAppProfile { exe } => {
                if ctx.nvidia.app_profile_exists(exe)? {
                    return Ok(());
                }
                ctx.nvidia.create_app_profile(exe)
            }
            Atom::NvSetting {
                profile,
                setting,
                value,
                ..
            } => ctx.nvidia.set(profile, *setting, *value),
        }
    }

    pub fn restore(&self, ctx: &SysCtx, original: &Original) -> SysResult<()> {
        match (self, original) {
            (
                Atom::Registry {
                    hive, path, name, ..
                },
                Original::Registry {
                    value,
                    created_keys,
                },
            ) => {
                // Only touch what differs: after a failed write, the target is
                // usually unchanged and writing to it again would fail the same way.
                if ctx.registry.get(*hive, path, name)? != *value {
                    match value {
                        Some(data) => ctx.registry.set(*hive, path, name, data)?,
                        None => ctx.registry.delete_value(*hive, path, name)?,
                    }
                }
                for key in created_keys {
                    if ctx.registry.key_exists(*hive, key)? {
                        ctx.registry.delete_key_if_empty(*hive, key)?;
                    }
                }
                Ok(())
            }
            (Atom::Service { name, .. }, Original::Service { state }) => {
                let Some(before) = state else { return Ok(()) };
                let Some(now) = ctx.services.query(name)? else {
                    return Ok(());
                };
                if now.start != before.start {
                    ctx.services.set_start(name, before.start)?;
                }
                if before.running && !now.running {
                    ctx.services.start(name)?;
                }
                Ok(())
            }
            (Atom::PowerPlan { guid, .. }, Original::PowerPlan { existed }) => {
                // Only a plan Apex Tune created is ever deleted.
                if !existed && ctx.power.exists(guid)? {
                    ctx.power.delete(guid)?;
                }
                Ok(())
            }
            (
                Atom::PowerSetting {
                    scheme,
                    subgroup,
                    setting,
                    source,
                    ..
                },
                Original::PowerSetting { value },
            ) => {
                let Some(before) = value else { return Ok(()) };
                if ctx.power.read(scheme, subgroup, setting, *source)? == Some(*before) {
                    return Ok(());
                }
                ctx.power.write(scheme, subgroup, setting, *source, *before)
            }
            (Atom::ActivePowerPlan { .. }, Original::ActivePowerPlan { guid }) => {
                if ctx.power.active()?.eq_ignore_ascii_case(guid) {
                    return Ok(());
                }
                ctx.power.set_active(guid)
            }
            (Atom::LogonTask { path, .. }, Original::LogonTask { xml, enabled }) => match xml {
                None => ctx.tasks.delete(path),
                Some(xml) => {
                    ctx.tasks.register(path, xml)?;
                    ctx.tasks.set_enabled(path, *enabled)
                }
            },
            (
                Atom::AppRemoved {
                    name, full_name, ..
                },
                Original::AppRemoved { installed },
            ) => {
                if *installed && !Self::app_installed(ctx, name)? {
                    ctx.apps.restore(full_name)?;
                }
                Ok(())
            }
            (Atom::Spi { flag, .. }, Original::Spi { value }) => {
                if ctx.desktop.spi_get(*flag)? == *value {
                    return Ok(());
                }
                ctx.desktop.spi_set(*flag, *value)
            }
            (Atom::DisplayRefresh { device, .. }, Original::DisplayRefresh { hz }) => {
                if Self::current_hz(ctx, device)? == Some(*hz) {
                    return Ok(());
                }
                ctx.desktop.set_refresh(device, *hz)
            }
            (Atom::Dns { adapter, .. }, Original::Dns { v4, v6 }) => {
                if Self::current_dns(ctx, adapter)? == (v4.clone(), v6.clone()) {
                    return Ok(());
                }
                ctx.network.set_dns(adapter, v4, v6)
            }
            (Atom::NvAppProfile { exe }, Original::NvAppProfile { existed }) => {
                if !existed {
                    ctx.nvidia.delete_app_profile(exe)?;
                }
                Ok(())
            }
            (
                Atom::NvSetting {
                    profile, setting, ..
                },
                Original::NvSetting { value },
            ) => {
                if Self::nv_get(ctx, profile, *setting)? == *value {
                    return Ok(());
                }
                match value {
                    Some(v) => ctx.nvidia.set(profile, *setting, *v),
                    None => ctx.nvidia.clear(profile, *setting),
                }
            }
            (Atom::TaskEnabled { path, .. }, Original::TaskEnabled { enabled }) => {
                match (enabled, ctx.tasks.get(path)?) {
                    (Some(before), Some((_, now))) if *before != now => {
                        ctx.tasks.set_enabled(path, *before)
                    }
                    _ => Ok(()),
                }
            }
            _ => Err(SysError::new(
                SysErrorKind::Other,
                "restore",
                self.label(),
                "backup record doesn't match this change",
            )),
        }
    }

    /// Whether the target state is in place. A missing service counts as in
    /// place: there's nothing to change.
    pub fn is_applied(&self, ctx: &SysCtx) -> SysResult<bool> {
        match self {
            Atom::Registry {
                hive,
                path,
                name,
                target,
            } => {
                let current = ctx.registry.get(*hive, path, name)?;
                Ok(match target {
                    RegTarget::Set(data) => current.as_ref() == Some(data),
                    RegTarget::Absent => current.is_none(),
                })
            }
            Atom::Service { name, start, .. } => {
                Ok(ctx.services.query(name)?.is_none_or(|s| s.start == *start))
            }
            Atom::PowerPlan { guid, .. } => ctx.power.exists(guid),
            Atom::PowerSetting {
                scheme,
                subgroup,
                setting,
                source,
                value,
                ..
            } => Ok(ctx.power.read(scheme, subgroup, setting, *source)? == Some(*value)),
            Atom::ActivePowerPlan { guid } => Ok(ctx.power.active()?.eq_ignore_ascii_case(guid)),
            Atom::LogonTask {
                path,
                command,
                arguments,
                ..
            } => Ok(match ctx.tasks.get(path)? {
                Some((xml, enabled)) => {
                    enabled
                        && task_xml::exec_action(&xml).is_some_and(|(c, a)| {
                            c.eq_ignore_ascii_case(command) && a == *arguments
                        })
                }
                None => false,
            }),
            Atom::TaskEnabled { path, enabled } => {
                Ok(ctx.tasks.get(path)?.is_none_or(|(_, now)| now == *enabled))
            }
            Atom::AppRemoved { name, .. } => Ok(!Self::app_installed(ctx, name)?),
            Atom::Spi { flag, value } => Ok(ctx.desktop.spi_get(*flag)? == *value),
            Atom::DisplayRefresh { device, hz, .. } => {
                Ok(Self::current_hz(ctx, device)? == Some(*hz))
            }
            Atom::Dns {
                adapter, v4, v6, ..
            } => Ok(Self::current_dns(ctx, adapter)? == (v4.clone(), v6.clone())),
            Atom::NvAppProfile { exe } => ctx.nvidia.app_profile_exists(exe),
            Atom::NvSetting {
                profile,
                setting,
                value,
                ..
            } => Ok(Self::nv_get(ctx, profile, *setting)? == Some(*value)),
        }
    }

    /// Whether the current state equals a captured original (used to verify a restore).
    pub fn matches_original(&self, ctx: &SysCtx, original: &Original) -> SysResult<bool> {
        match (self, original) {
            (
                Atom::Registry {
                    hive, path, name, ..
                },
                Original::Registry { value, .. },
            ) => Ok(ctx.registry.get(*hive, path, name)? == *value),
            (Atom::Service { name, .. }, Original::Service { state }) => Ok(match state {
                None => true,
                Some(before) => ctx
                    .services
                    .query(name)?
                    .is_none_or(|now| now.start == before.start),
            }),
            (Atom::PowerPlan { guid, .. }, Original::PowerPlan { existed }) => {
                Ok(ctx.power.exists(guid)? == *existed)
            }
            (
                Atom::PowerSetting {
                    scheme,
                    subgroup,
                    setting,
                    source,
                    ..
                },
                Original::PowerSetting { value },
            ) => Ok(value.is_none()
                || !ctx.power.exists(scheme)?
                || ctx.power.read(scheme, subgroup, setting, *source)? == *value),
            (Atom::ActivePowerPlan { .. }, Original::ActivePowerPlan { guid }) => {
                Ok(ctx.power.active()?.eq_ignore_ascii_case(guid))
            }
            (Atom::LogonTask { path, .. }, Original::LogonTask { xml, .. }) => {
                Ok(ctx.tasks.get(path)?.is_some() == xml.is_some())
            }
            (Atom::AppRemoved { name, .. }, Original::AppRemoved { installed }) => {
                Ok(Self::app_installed(ctx, name)? == *installed)
            }
            (Atom::Spi { flag, .. }, Original::Spi { value }) => {
                Ok(ctx.desktop.spi_get(*flag)? == *value)
            }
            (Atom::DisplayRefresh { device, .. }, Original::DisplayRefresh { hz }) => {
                Ok(Self::current_hz(ctx, device)?.is_none_or(|now| now == *hz))
            }
            (Atom::Dns { adapter, .. }, Original::Dns { v4, v6 }) => {
                Ok(Self::current_dns(ctx, adapter)? == (v4.clone(), v6.clone()))
            }
            (Atom::NvAppProfile { exe }, Original::NvAppProfile { existed }) => {
                Ok(ctx.nvidia.app_profile_exists(exe)? == *existed)
            }
            (
                Atom::NvSetting {
                    profile, setting, ..
                },
                Original::NvSetting { value },
            ) => Ok(Self::nv_get(ctx, profile, *setting)? == *value),
            (Atom::TaskEnabled { path, .. }, Original::TaskEnabled { enabled }) => {
                Ok(match enabled {
                    None => true,
                    Some(before) => ctx.tasks.get(path)?.is_none_or(|(_, now)| now == *before),
                })
            }
            _ => Ok(false),
        }
    }

    pub fn preview(&self, ctx: &SysCtx) -> SysResult<ChangePreview> {
        let target = self.label();
        match self {
            Atom::Registry {
                hive,
                path,
                name,
                target: wanted,
            } => {
                let from = ctx
                    .registry
                    .get(*hive, path, name)?
                    .map_or(ABSENT.into(), |d| d.to_string());
                let to = match wanted {
                    RegTarget::Set(data) => data.to_string(),
                    RegTarget::Absent => "(removed)".into(),
                };
                Ok(ChangePreview { target, from, to })
            }
            Atom::Service { name, start, stop } => {
                let from = ctx
                    .services
                    .query(name)?
                    .map_or("Not installed".into(), |s| s.start.to_string());
                let to = if *stop {
                    format!("{start}, stopped now")
                } else {
                    start.to_string()
                };
                Ok(ChangePreview { target, from, to })
            }
            Atom::PowerPlan { guid, .. } => Ok(ChangePreview {
                target,
                from: if ctx.power.exists(guid)? {
                    "Installed"
                } else {
                    "Not installed"
                }
                .into(),
                to: "Created".into(),
            }),
            Atom::PowerSetting {
                scheme,
                subgroup,
                setting,
                source,
                unit,
                value,
                ..
            } => Ok(ChangePreview {
                target,
                from: ctx
                    .power
                    .read(scheme, subgroup, setting, *source)?
                    .map_or("Plan not created yet".into(), |v| unit.format(v)),
                to: unit.format(*value),
            }),
            Atom::ActivePowerPlan { guid } => {
                let schemes = ctx.power.schemes()?;
                let name_of = |g: &str| {
                    schemes
                        .iter()
                        .find(|s| s.guid.eq_ignore_ascii_case(g))
                        .map_or_else(|| g.to_owned(), |s| s.name.clone())
                };
                Ok(ChangePreview {
                    target,
                    from: name_of(&ctx.power.active()?),
                    to: name_of(guid),
                })
            }
            Atom::LogonTask {
                command, arguments, ..
            } => Ok(ChangePreview {
                target,
                from: if ctx.tasks.get(&self.label())?.is_some() {
                    "Exists"
                } else {
                    "Not set up"
                }
                .into(),
                to: format!("Runs {command} {arguments} at sign-in"),
            }),
            Atom::TaskEnabled { path, enabled } => Ok(ChangePreview {
                target,
                from: match ctx.tasks.get(path)? {
                    None => "Not present".into(),
                    Some((_, true)) => "Enabled".into(),
                    Some((_, false)) => "Disabled".into(),
                },
                to: if *enabled { "Enabled" } else { "Disabled" }.into(),
            }),
            Atom::AppRemoved { name, .. } => Ok(ChangePreview {
                target,
                from: if Self::app_installed(ctx, name)? {
                    "Installed"
                } else {
                    "Not installed"
                }
                .into(),
                to: "Removed for your account".into(),
            }),
            Atom::Spi { flag, value } => Ok(ChangePreview {
                target,
                from: if ctx.desktop.spi_get(*flag)? {
                    "On"
                } else {
                    "Off"
                }
                .into(),
                to: if *value { "On" } else { "Off" }.into(),
            }),
            Atom::DisplayRefresh { device, hz, .. } => Ok(ChangePreview {
                target,
                from: Self::current_hz(ctx, device)?
                    .map_or("Unknown".into(), |h| format!("{h} Hz")),
                to: format!("{hz} Hz"),
            }),
            Atom::Dns { adapter, v4, .. } => {
                let (now, _) = Self::current_dns(ctx, adapter)?;
                let show = |l: &[String]| {
                    if l.is_empty() {
                        "Automatic".to_owned()
                    } else {
                        l.join(", ")
                    }
                };
                Ok(ChangePreview {
                    target,
                    from: show(&now),
                    to: show(v4),
                })
            }
            Atom::NvAppProfile { exe } => Ok(ChangePreview {
                target,
                from: if ctx.nvidia.app_profile_exists(exe)? {
                    "Exists"
                } else {
                    "None"
                }
                .into(),
                to: "Profile for this program".into(),
            }),
            Atom::NvSetting {
                profile,
                setting,
                value_label,
                ..
            } => Ok(ChangePreview {
                target,
                from: Self::nv_get(ctx, profile, *setting)?
                    .map_or("Driver default".into(), |v| format!("0x{v:X}")),
                to: value_label.clone(),
            }),
        }
    }
}

/// Keys on `path` that don't exist yet, deepest first.
fn missing_keys(ctx: &SysCtx, hive: Hive, path: &str) -> SysResult<Vec<String>> {
    let parts: Vec<&str> = path.split('\\').filter(|p| !p.is_empty()).collect();
    let mut missing = Vec::new();
    for depth in (1..=parts.len()).rev() {
        let key = parts[..depth].join("\\");
        if ctx.registry.key_exists(hive, &key)? {
            break;
        }
        missing.push(key);
    }
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::fake::FakeSystem;

    const PATH: &str = r"Software\Apex\Test";

    #[test]
    fn registry_round_trip_restores_existing_value() {
        let sys = FakeSystem::new();
        sys.registry
            .seed(Hive::CurrentUser, PATH, "v", RegData::Dword(7));
        let atom = Atom::reg_dword(Hive::CurrentUser, PATH, "v", 1);

        let original = atom.capture(&sys.ctx).unwrap();
        atom.write(&sys.ctx).unwrap();
        assert!(atom.is_applied(&sys.ctx).unwrap());
        atom.restore(&sys.ctx, &original).unwrap();
        assert_eq!(
            sys.registry.value(Hive::CurrentUser, PATH, "v"),
            Some(RegData::Dword(7))
        );
    }

    #[test]
    fn registry_restore_removes_created_value_and_keys() {
        let sys = FakeSystem::new();
        sys.registry.seed_key(Hive::CurrentUser, "Software");
        let atom = Atom::reg_dword(Hive::CurrentUser, PATH, "v", 1);

        let original = atom.capture(&sys.ctx).unwrap();
        assert_eq!(
            original,
            Original::Registry {
                value: None,
                created_keys: vec![PATH.into(), r"Software\Apex".into()]
            }
        );
        atom.write(&sys.ctx).unwrap();
        atom.restore(&sys.ctx, &original).unwrap();
        assert!(!sys.registry.has_key(Hive::CurrentUser, r"Software\Apex"));
        assert!(sys.registry.has_key(Hive::CurrentUser, "Software"));
    }

    #[test]
    fn absent_target_deletes_and_restores_value() {
        let sys = FakeSystem::new();
        sys.registry
            .seed(Hive::LocalMachine, PATH, "p", RegData::Sz("x".into()));
        let atom = Atom::reg_absent(Hive::LocalMachine, PATH, "p");
        let original = atom.capture(&sys.ctx).unwrap();
        atom.write(&sys.ctx).unwrap();
        assert!(atom.is_applied(&sys.ctx).unwrap());
        atom.restore(&sys.ctx, &original).unwrap();
        assert_eq!(
            sys.registry.value(Hive::LocalMachine, PATH, "p"),
            Some(RegData::Sz("x".into()))
        );
    }

    #[test]
    fn service_round_trip_restores_start_type_and_running_state() {
        let sys = FakeSystem::new();
        sys.services
            .install("DiagTrack", StartType::Automatic, true);
        let atom = Atom::service("DiagTrack", StartType::Disabled, true);

        let original = atom.capture(&sys.ctx).unwrap();
        atom.write(&sys.ctx).unwrap();
        assert_eq!(
            sys.services.state("DiagTrack"),
            Some(ServiceState {
                start: StartType::Disabled,
                running: false
            })
        );
        atom.restore(&sys.ctx, &original).unwrap();
        assert_eq!(
            sys.services.state("DiagTrack"),
            Some(ServiceState {
                start: StartType::Automatic,
                running: true
            })
        );
    }

    #[test]
    fn missing_service_is_a_no_op() {
        let sys = FakeSystem::new();
        let atom = Atom::service("Fax", StartType::Disabled, false);
        let original = atom.capture(&sys.ctx).unwrap();
        atom.write(&sys.ctx).unwrap();
        assert!(atom.is_applied(&sys.ctx).unwrap());
        atom.restore(&sys.ctx, &original).unwrap();
    }

    #[test]
    fn preview_shows_before_and_after() {
        let sys = FakeSystem::new();
        let atom = Atom::reg_dword(Hive::LocalMachine, PATH, "HwSchMode", 2);
        let preview = atom.preview(&sys.ctx).unwrap();
        assert_eq!(preview.from, "(not set)");
        assert_eq!(preview.to, "2 (0x2)");
    }
}
