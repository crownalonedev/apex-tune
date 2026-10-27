//! Network adapters (GetAdaptersAddresses) and DNS (SetInterfaceDnsSettings).

use windows::Win32::NetworkManagement::IpHelper::{
    DNS_INTERFACE_SETTINGS, DNS_INTERFACE_SETTINGS_VERSION1, DNS_SETTING_IPV6,
    DNS_SETTING_NAMESERVER, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    IP_ADAPTER_ADDRESSES_LH, SetInterfaceDnsSettings,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::AF_UNSPEC;
use windows::core::{GUID, PWSTR};

use crate::sys::{Adapter, Network, SysError, SysErrorKind, SysResult};

const IF_TYPE_ETHERNET: u32 = 6;
const IF_TYPE_WIFI: u32 = 71;

fn parse_guid(s: &str) -> SysResult<GUID> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    u128::from_str_radix(&hex, 16)
        .ok()
        .filter(|_| hex.len() == 32)
        .map(GUID::from_u128)
        .ok_or_else(|| {
            SysError::new(
                SysErrorKind::Other,
                "change DNS",
                s,
                "not a valid adapter GUID",
            )
        })
}

pub struct WinNetwork;

impl Network for WinNetwork {
    fn adapters(&self) -> SysResult<Vec<Adapter>> {
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
        let mut size = 16 * 1024u32;
        let mut buf: Vec<u64>;
        loop {
            buf = vec![0u64; (size as usize).div_ceil(8)];
            // SAFETY: the buffer holds `size` bytes, 8-byte aligned.
            let code = unsafe {
                GetAdaptersAddresses(
                    AF_UNSPEC.0 as u32,
                    flags,
                    None,
                    Some(buf.as_mut_ptr().cast()),
                    &mut size,
                )
            };
            match code {
                0 => break,
                111 => continue, // ERROR_BUFFER_OVERFLOW: `size` now holds what's needed.
                other => {
                    return Err(SysError::new(
                        SysErrorKind::Other,
                        "list network adapters",
                        "",
                        format!("error {other}"),
                    ));
                }
            }
        }
        let mut out = Vec::new();
        let mut ptr = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !ptr.is_null() {
            // SAFETY: a linked list inside `buf`, which outlives this loop.
            let a = unsafe { &*ptr };
            if a.IfType == IF_TYPE_ETHERNET || a.IfType == IF_TYPE_WIFI {
                // SAFETY: AdapterName is a NUL-terminated ANSI string inside `buf`.
                let guid = unsafe { a.AdapterName.to_string() }.unwrap_or_default();
                let wide = |p: PWSTR| {
                    // SAFETY: FriendlyName and Description are NUL-terminated strings inside `buf`.
                    unsafe { p.to_string() }.unwrap_or_default()
                };
                let description = wide(a.Description);
                // Hyper-V, VPN and other virtual Ethernet adapters aren't worth tuning.
                let virtual_adapter = [
                    "virtual",
                    "hyper-v",
                    "vpn",
                    "tap-",
                    "wan miniport",
                    "loopback",
                ]
                .iter()
                .any(|m| description.to_ascii_lowercase().contains(m));
                if !virtual_adapter {
                    out.push(Adapter {
                        guid,
                        name: wide(a.FriendlyName),
                        description,
                        wired: a.IfType == IF_TYPE_ETHERNET,
                        connected: a.OperStatus == IfOperStatusUp,
                    });
                }
            }
            ptr = a.Next;
        }
        Ok(out)
    }

    fn set_dns(&self, guid: &str, v4: &[String], v6: &[String]) -> SysResult<()> {
        let id = parse_guid(guid)?;
        for (servers, extra) in [(v4, 0), (v6, DNS_SETTING_IPV6)] {
            let mut list: Vec<u16> = servers.join(",").encode_utf16().chain([0]).collect();
            let settings = DNS_INTERFACE_SETTINGS {
                Version: DNS_INTERFACE_SETTINGS_VERSION1,
                Flags: u64::from(DNS_SETTING_NAMESERVER | extra),
                NameServer: PWSTR(list.as_mut_ptr()),
                ..Default::default()
            };
            // SAFETY: `settings` and its string are valid for the call.
            let code = unsafe { SetInterfaceDnsSettings(id, &settings) };
            if code.0 != 0 {
                return Err(SysError::new(
                    SysErrorKind::Other,
                    "change DNS",
                    guid,
                    windows::core::Error::from(code.to_hresult()).message(),
                ));
            }
        }
        Ok(())
    }
}
