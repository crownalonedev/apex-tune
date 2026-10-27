//! Network tweaks. Online games use UDP, so TCP tweaks barely affect them;
//! the descriptions say where the effect actually is.

use super::{Applicability, Atom, Category, Refresh, Restart, Risk, StaticTweak, Tweak, TweakMeta};
use crate::sys::Hive::LocalMachine as HKLM;
use crate::sys::{Hive, RegData, SysCtx, SysResult};

const INTERFACES: &str = r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";
const NET_CLASS: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";

pub fn network_throttling_off() -> StaticTweak {
    StaticTweak {
        meta: TweakMeta {
            id: "network_throttling_off",
            name: "No network throttling while audio plays",
            description: "Windows limits non-multimedia network packets to protect audio playback. Removing the \
                          limit helps downloads and streaming while music or voice chat runs. Small effect on game \
                          traffic itself.",
            category: Category::Network,
            risk: Risk::Moderate,
            restart: Restart::Reboot,
            recommended: false,
        },
        atoms: || {
            vec![Atom::reg_dword(
                HKLM,
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile",
                "NetworkThrottlingIndex",
                0xFFFF_FFFF,
            )]
        },
        refresh: Refresh::Nothing,
    }
}

/// Immediate TCP acknowledgements on every physical adapter.
pub struct TcpAckFrequency {
    meta: TweakMeta,
}

impl TcpAckFrequency {
    pub fn new() -> Self {
        Self {
            meta: TweakMeta {
                id: "tcp_ack_frequency",
                name: "Acknowledge TCP packets immediately",
                description: "Turns off delayed ACKs on your network adapters. Only affects TCP: Valorant and \
                              Fortnite gameplay is UDP and Minecraft already disables Nagle itself, so expect little \
                              difference in those. Can help older TCP-based games.",
                category: Category::Network,
                risk: Risk::Advanced,
                restart: Restart::Reboot,
                recommended: false,
            },
        }
    }
}

impl Default for TcpAckFrequency {
    fn default() -> Self {
        Self::new()
    }
}

impl Tweak for TcpAckFrequency {
    fn meta(&self) -> &TweakMeta {
        &self.meta
    }

    fn applicable(&self, ctx: &SysCtx) -> SysResult<Applicability> {
        Ok(if ctx.network.adapters()?.is_empty() {
            Applicability::NotApplicable("No physical network adapter found.".into())
        } else {
            Applicability::Applicable
        })
    }

    fn atoms(&self, ctx: &SysCtx) -> SysResult<Vec<Atom>> {
        Ok(ctx
            .network
            .adapters()?
            .into_iter()
            .map(|a| {
                Atom::reg_dword(
                    HKLM,
                    &format!(r"{INTERFACES}\{}", a.guid),
                    "TcpAckFrequency",
                    1,
                )
            })
            .collect())
    }
}

/// Energy Efficient Ethernet and vendor "green" modes on wired adapters.
pub struct EthernetPowerSaving {
    meta: TweakMeta,
}

impl EthernetPowerSaving {
    pub fn new() -> Self {
        Self {
            meta: TweakMeta {
                id: "ethernet_power_saving_off",
                name: "Turn off Ethernet power saving",
                description: "Disables Energy Efficient Ethernet and Realtek Green Ethernet on wired adapters. Some \
                              drivers drop into low-power mode mid-game and cause short stutters or ping spikes. \
                              Uses a few watts more.",
                category: Category::Network,
                risk: Risk::Moderate,
                restart: Restart::Reboot,
                recommended: false,
            },
        }
    }

    /// (driver key path, value name) for every power-saving option a wired adapter exposes.
    fn targets(ctx: &SysCtx) -> SysResult<Vec<(String, String)>> {
        let wired: Vec<String> = ctx
            .network
            .adapters()?
            .into_iter()
            .filter(|a| a.wired)
            .map(|a| a.guid.to_ascii_lowercase())
            .collect();
        let mut out = Vec::new();
        for sub in ctx.registry.subkeys(Hive::LocalMachine, NET_CLASS)? {
            if sub.len() != 4 || !sub.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let key = format!(r"{NET_CLASS}\{sub}");
            let id = match ctx
                .registry
                .get(Hive::LocalMachine, &key, "NetCfgInstanceId")?
            {
                Some(RegData::Sz(s)) => s.to_ascii_lowercase(),
                _ => continue,
            };
            if !wired.contains(&id) {
                continue;
            }
            for name in ["*EEE", "EnableGreenEthernet", "AdvancedEEE"] {
                if ctx.registry.get(Hive::LocalMachine, &key, name)?.is_some() {
                    out.push((key.clone(), name.to_owned()));
                }
            }
        }
        Ok(out)
    }
}

impl Default for EthernetPowerSaving {
    fn default() -> Self {
        Self::new()
    }
}

impl Tweak for EthernetPowerSaving {
    fn meta(&self) -> &TweakMeta {
        &self.meta
    }

    fn applicable(&self, ctx: &SysCtx) -> SysResult<Applicability> {
        Ok(if Self::targets(ctx)?.is_empty() {
            Applicability::NotApplicable("No wired adapter with a power-saving option.".into())
        } else {
            Applicability::Applicable
        })
    }

    fn atoms(&self, ctx: &SysCtx) -> SysResult<Vec<Atom>> {
        // Adapter advanced properties are strings.
        Ok(Self::targets(ctx)?
            .into_iter()
            .map(|(key, name)| Atom::reg_sz(HKLM, &key, &name, "0"))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::Adapter;
    use crate::sys::fake::FakeSystem;
    use crate::tweaks::TweakState;

    fn adapter(guid: &str, wired: bool) -> Adapter {
        Adapter {
            guid: guid.into(),
            name: "Ethernet".into(),
            description: "Realtek".into(),
            wired,
            connected: true,
        }
    }

    #[test]
    fn ethernet_power_saving_targets_only_wired_adapters_with_the_option() {
        let sys = FakeSystem::new();
        sys.network
            .adapters
            .lock()
            .unwrap()
            .extend([adapter("{AAA}", true), adapter("{BBB}", false)]);
        let wired = format!(r"{NET_CLASS}\0001");
        let wifi = format!(r"{NET_CLASS}\0002");
        sys.registry.seed(
            Hive::LocalMachine,
            &wired,
            "NetCfgInstanceId",
            RegData::Sz("{AAA}".into()),
        );
        sys.registry
            .seed(Hive::LocalMachine, &wired, "*EEE", RegData::Sz("1".into()));
        sys.registry.seed(
            Hive::LocalMachine,
            &wifi,
            "NetCfgInstanceId",
            RegData::Sz("{BBB}".into()),
        );
        sys.registry
            .seed(Hive::LocalMachine, &wifi, "*EEE", RegData::Sz("1".into()));

        let tweak = EthernetPowerSaving::new();
        let atoms = tweak.atoms(&sys.ctx).unwrap();
        assert_eq!(atoms.len(), 1);
        assert_eq!(tweak.detect(&sys.ctx).unwrap(), TweakState::NotApplied);
    }

    #[test]
    fn not_applicable_without_adapters() {
        let sys = FakeSystem::new();
        assert!(matches!(
            TcpAckFrequency::new().detect(&sys.ctx).unwrap(),
            TweakState::NotApplicable(_)
        ));
        assert!(matches!(
            EthernetPowerSaving::new().detect(&sys.ctx).unwrap(),
            TweakState::NotApplicable(_)
        ));
    }
}
