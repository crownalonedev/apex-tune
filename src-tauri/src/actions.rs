//! One-off, journaled actions that aren't on/off tweaks: the DNS switcher
//! and pausing Windows Update.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::sys::{Hive, RegData, SysCtx, SysResult};
use crate::tweaks::atom::Atom;

// ---- DNS ------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DnsPreset {
    Automatic,
    Cloudflare,
    Google,
    Quad9,
}

impl DnsPreset {
    pub fn servers(self) -> (Vec<String>, Vec<String>) {
        let s = |l: &[&str]| l.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        match self {
            DnsPreset::Automatic => (Vec::new(), Vec::new()),
            DnsPreset::Cloudflare => (
                s(&["1.1.1.1", "1.0.0.1"]),
                s(&["2606:4700:4700::1111", "2606:4700:4700::1001"]),
            ),
            DnsPreset::Google => (
                s(&["8.8.8.8", "8.8.4.4"]),
                s(&["2001:4860:4860::8888", "2001:4860:4860::8844"]),
            ),
            DnsPreset::Quad9 => (
                s(&["9.9.9.9", "149.112.112.112"]),
                s(&["2620:fe::fe", "2620:fe::9"]),
            ),
        }
    }

    /// Which preset a server list matches, if any.
    pub fn detect(v4: &[String]) -> Option<DnsPreset> {
        [
            DnsPreset::Automatic,
            DnsPreset::Cloudflare,
            DnsPreset::Google,
            DnsPreset::Quad9,
        ]
        .into_iter()
        .find(|p| p.servers().0 == v4)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDns {
    pub guid: String,
    pub name: String,
    pub description: String,
    pub wired: bool,
    pub connected: bool,
    pub servers: Vec<String>,
    /// `None` for a custom list someone set by hand.
    pub preset: Option<DnsPreset>,
}

fn dns_of(ctx: &SysCtx, guid: &str) -> SysResult<Vec<String>> {
    let path = format!(r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{guid}");
    Ok(
        match ctx.registry.get(Hive::LocalMachine, &path, "NameServer")? {
            Some(RegData::Sz(s)) => s
                .split([',', ' '])
                .filter(|x| !x.is_empty())
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        },
    )
}

pub fn dns_status(ctx: &SysCtx) -> SysResult<Vec<AdapterDns>> {
    ctx.network
        .adapters()?
        .into_iter()
        .map(|a| {
            let servers = dns_of(ctx, &a.guid)?;
            Ok(AdapterDns {
                preset: DnsPreset::detect(&servers),
                guid: a.guid,
                name: a.name,
                description: a.description,
                wired: a.wired,
                connected: a.connected,
                servers,
            })
        })
        .collect()
}

pub fn dns_atom(ctx: &SysCtx, guid: &str, preset: DnsPreset) -> SysResult<Option<Atom>> {
    let Some(adapter) = ctx
        .network
        .adapters()?
        .into_iter()
        .find(|a| a.guid.eq_ignore_ascii_case(guid))
    else {
        return Ok(None);
    };
    let (v4, v6) = preset.servers();
    Ok(Some(Atom::Dns {
        adapter: adapter.guid,
        adapter_name: adapter.name,
        v4,
        v6,
    }))
}

pub fn dns_action(guid: &str) -> String {
    format!("dns.{}", guid.to_ascii_lowercase())
}

// ---- Pause Windows Update -------------------------------------------------------

pub const PAUSE_ACTION: &str = "updates.pause";
const UX_SETTINGS: &str = r"SOFTWARE\Microsoft\WindowsUpdate\UX\Settings";
/// Settings → Windows Update allows up to five weeks.
pub const MAX_PAUSE_WEEKS: u32 = 5;

fn iso(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// The values Settings → Windows Update → Pause writes.
pub fn pause_atoms(now: DateTime<Utc>, weeks: u32) -> Vec<Atom> {
    let start = iso(now);
    let end = iso(now + Duration::weeks(i64::from(weeks.clamp(1, MAX_PAUSE_WEEKS))));
    [
        ("PauseUpdatesStartTime", &start),
        ("PauseUpdatesExpiryTime", &end),
        ("PauseFeatureUpdatesStartTime", &start),
        ("PauseFeatureUpdatesEndTime", &end),
        ("PauseQualityUpdatesStartTime", &start),
        ("PauseQualityUpdatesEndTime", &end),
    ]
    .into_iter()
    .map(|(name, value)| Atom::reg_sz(Hive::LocalMachine, UX_SETTINGS, name, value))
    .collect()
}

/// When updates resume, if they're paused now.
pub fn paused_until(ctx: &SysCtx, now: DateTime<Utc>) -> SysResult<Option<String>> {
    Ok(
        match ctx
            .registry
            .get(Hive::LocalMachine, UX_SETTINGS, "PauseUpdatesExpiryTime")?
        {
            Some(RegData::Sz(s)) => DateTime::parse_from_rfc3339(&s)
                .ok()
                .filter(|t| *t > now)
                .map(|_| s),
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::journal::Journal;
    use crate::sys::Adapter;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::Engine;

    #[test]
    fn dns_switch_and_revert_restores_automatic() {
        let sys = FakeSystem::new();
        sys.network.adapters.lock().unwrap().push(Adapter {
            guid: "{ABC}".into(),
            name: "Ethernet".into(),
            description: "Intel".into(),
            wired: true,
            connected: true,
        });
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(sys.ctx.clone(), Journal::open(dir.path()).unwrap(), vec![]);
        assert_eq!(
            dns_status(&sys.ctx).unwrap()[0].preset,
            Some(DnsPreset::Automatic)
        );

        let atom = dns_atom(&sys.ctx, "{abc}", DnsPreset::Cloudflare)
            .unwrap()
            .unwrap();
        engine
            .apply_action(&dns_action("{ABC}"), "DNS: Cloudflare", vec![atom])
            .unwrap();
        let status = dns_status(&sys.ctx).unwrap();
        assert_eq!(status[0].preset, Some(DnsPreset::Cloudflare));
        assert_eq!(status[0].servers, vec!["1.1.1.1", "1.0.0.1"]);

        engine.revert_action(&dns_action("{ABC}")).unwrap();
        assert_eq!(
            dns_status(&sys.ctx).unwrap()[0].preset,
            Some(DnsPreset::Automatic)
        );
    }

    #[test]
    fn pausing_writes_the_settings_values_and_is_capped() {
        let now = DateTime::parse_from_rfc3339("2026-10-26T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let atoms = pause_atoms(now, 9);
        assert_eq!(atoms.len(), 6);
        let sys = FakeSystem::new();
        for a in &atoms {
            crate::guard::check_atom(a).unwrap();
            a.write(&sys.ctx).unwrap();
        }
        assert_eq!(
            paused_until(&sys.ctx, now).unwrap().as_deref(),
            Some("2026-11-30T12:00:00Z")
        );
        assert_eq!(
            paused_until(&sys.ctx, now + Duration::weeks(6)).unwrap(),
            None
        );
    }
}
