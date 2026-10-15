//! Schnittstellen auflisten (Kabel/WLAN), WLAN-Details lesen, Störfaktoren erkennen
//! (VPN, Akkubetrieb) und den Ruhezustand während der Messung verhindern.

use crate::model::{IfaceInfo, WifiInfo};
use crate::net::Bind;
use netdev::interface::types::InterfaceType;

const VIRTUAL: &[&str] = &["utun", "awdl", "llw", "bridge", "vmnet", "vboxnet", "docker", "veth", "anpi", "ap1", "gif", "stf"];
const VPN_HINTS: &[&str] = &["vpn", "wireguard", "openvpn", "tap-windows", "fortinet", "anyconnect", "globalprotect", "wintun", "tailscale", "zerotier", "nordlynx", "securepoint"];

fn kind_of(i: &netdev::Interface) -> &'static str {
    match i.if_type {
        InterfaceType::Wireless80211 => "wifi",
        InterfaceType::Ethernet
        | InterfaceType::GigabitEthernet
        | InterfaceType::FastEthernetT
        | InterfaceType::FastEthernetFx => "ethernet",
        _ => "other",
    }
}

fn is_vpn(i: &netdev::Interface) -> bool {
    let n = i.name.to_lowercase();
    let d = format!("{} {}", i.description.clone().unwrap_or_default(), i.friendly_name.clone().unwrap_or_default()).to_lowercase();
    matches!(i.if_type, InterfaceType::Tunnel | InterfaceType::Ppp)
        || ["utun", "ppp", "ipsec", "tun", "tap", "wg"].iter().any(|p| n.starts_with(p))
        || VPN_HINTS.iter().any(|h| d.contains(h))
}

fn usable_ipv4(i: &netdev::Interface) -> Option<std::net::Ipv4Addr> {
    i.ipv4.iter().map(|n| n.addr()).find(|a| !a.is_loopback() && !a.is_link_local() && !a.is_unspecified())
}

/// Kabel- und WLAN-Schnittstellen mit IPv4 (virtuelle/VPN-Adapter ausgeblendet).
pub fn list() -> Vec<IfaceInfo> {
    let default = netdev::get_default_interface().ok().map(|i| i.index);
    let mut out: Vec<IfaceInfo> = netdev::get_interfaces()
        .into_iter()
        .filter(|i| !i.is_loopback() && !is_vpn(i))
        .filter(|i| !VIRTUAL.iter().any(|p| i.name.starts_with(p)))
        // verbundene Adapter; WLAN auch unverbunden (damit „WLAN nicht verbunden“ angezeigt werden kann)
        .filter(|i| (i.is_up() && usable_ipv4(i).is_some()) || kind_of(i) == "wifi")
        .filter(|i| {
            // Windows listet viele unsichtbare Filter-/Miniport-Adapter; nur echte, laufende oder physische behalten
            let d = i.description.clone().unwrap_or_default().to_lowercase();
            !(d.contains("virtual") || d.contains("miniport") || d.contains("filter") || d.contains("hyper-v") || d.contains("bluetooth"))
        })
        .map(|i| {
            let kind = kind_of(&i).to_string();
            let ipv4 = usable_ipv4(&i).filter(|_| i.is_up());
            let label = match kind.as_str() {
                "wifi" => "WLAN",
                "ethernet" => "Kabel",
                _ => "Netzwerk",
            };
            let label = format!("{label} ({})", i.friendly_name.clone().unwrap_or_else(|| i.name.clone()));
            IfaceInfo {
                wifi: if kind == "wifi" && ipv4.is_some() { wifi_info(&i.name) } else { None },
                link_mbps: if ipv4.is_some() && kind == "ethernet" { link_mbps(&i) } else { None },
                gateway: i.gateway.as_ref().and_then(|g| g.ipv4.first()).map(|g| g.to_string()),
                is_default: Some(i.index) == default,
                name: i.name,
                label,
                index: i.index,
                kind,
                ipv4: ipv4.map(|a| a.to_string()),
            }
        })
        .collect();
    // Verbundene zuerst, Standard-Schnittstelle ganz oben
    out.sort_by_key(|i| (!i.is_default, i.ipv4.is_none(), i.kind != "ethernet"));
    out.dedup_by(|a, b| a.name == b.name);
    out
}

pub fn find(name: &str) -> Option<IfaceInfo> {
    let all = list();
    if name.is_empty() {
        return all.into_iter().find(|i| i.is_default && i.ipv4.is_some());
    }
    all.into_iter().find(|i| i.name == name)
}

pub fn bind_for(i: &IfaceInfo) -> Bind {
    Bind { ip: i.ipv4.as_ref().and_then(|s| s.parse().ok()), index: Some(i.index) }
}

fn link_mbps(i: &netdev::Interface) -> Option<u64> {
    i.transmit_speed.or(i.receive_speed).map(|bps| bps / 1_000_000).filter(|m| *m > 0)
}

/// Aktive VPN-/Tunnel-Adapter mit IPv4 (z. B. Praxis-/TI-VPN): Internetmessung misst dann evtl. den Tunnel.
pub fn active_vpns() -> Vec<String> {
    netdev::get_interfaces()
        .into_iter()
        .filter(|i| i.is_up() && is_vpn(i) && usable_ipv4(i).is_some())
        .map(|i| i.friendly_name.or(i.description).unwrap_or(i.name))
        .collect()
}

// ----------------------------------------------------------------- WLAN-Details

#[cfg(target_os = "macos")]
pub fn wifi_info(name: &str) -> Option<WifiInfo> {
    use objc2_core_wlan::{CWChannelBand, CWChannelWidth, CWWiFiClient};
    use objc2_foundation::NSString;
    unsafe {
        let client = CWWiFiClient::sharedWiFiClient();
        let w = client.interfaceWithName(Some(&NSString::from_str(name)))?;
        let rssi = w.rssiValue() as i32;
        let ch = w.wlanChannel();
        let band = ch.as_ref().map(|c| c.channelBand()).and_then(|b| match b {
            CWChannelBand::Band2GHz => Some("2,4 GHz".to_string()),
            CWChannelBand::Band5GHz => Some("5 GHz".to_string()),
            CWChannelBand::Band6GHz => Some("6 GHz".to_string()),
            _ => None,
        });
        let width = ch.as_ref().map(|c| c.channelWidth()).and_then(|w| match w {
            CWChannelWidth::Width20MHz => Some(20),
            CWChannelWidth::Width40MHz => Some(40),
            CWChannelWidth::Width80MHz => Some(80),
            CWChannelWidth::Width160MHz => Some(160),
            _ => None,
        });
        Some(WifiInfo {
            // SSID/BSSID liefert macOS nur mit Ortungsdienste-Freigabe, sonst None
            ssid: w.ssid().map(|s| s.to_string()),
            bssid: w.bssid().map(|s| s.to_string()),
            rssi_dbm: (rssi != 0).then_some(rssi),
            noise_dbm: Some(w.noiseMeasurement() as i32).filter(|n| *n != 0),
            channel: ch.as_ref().map(|c| c.channelNumber() as u32).filter(|c| *c > 0),
            band,
            width_mhz: width,
            tx_rate_mbps: Some(w.transmitRate()).filter(|r| *r > 0.0),
            quality_pct: None,
        })
    }
}

/// WLAN-API (wlanapi.dll) erst zur Laufzeit laden: Auf Windows Server ohne Feature „WLAN-Dienst“
/// fehlt die DLL – fest verknüpft würde das Programm dann gar nicht starten.
#[cfg(windows)]
mod wlanapi {
    use core::ffi::c_void;
    use std::sync::OnceLock;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::WiFi::{WLAN_INTERFACE_INFO_LIST, WLAN_INTF_OPCODE};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW};
    use windows::core::{GUID, s, w};

    pub struct Api {
        pub open: unsafe extern "system" fn(u32, *const c_void, *mut u32, *mut HANDLE) -> u32,
        pub enum_interfaces: unsafe extern "system" fn(HANDLE, *const c_void, *mut *mut WLAN_INTERFACE_INFO_LIST) -> u32,
        pub query: unsafe extern "system" fn(HANDLE, *const GUID, WLAN_INTF_OPCODE, *const c_void, *mut u32, *mut *mut c_void, *mut c_void) -> u32,
        pub free: unsafe extern "system" fn(*const c_void),
        pub close: unsafe extern "system" fn(HANDLE, *const c_void) -> u32,
    }

    /// `None`, wenn wlanapi.dll fehlt – dann gibt es schlicht keine WLAN-Details.
    pub fn get() -> Option<&'static Api> {
        static API: OnceLock<Option<Api>> = OnceLock::new();
        API.get_or_init(|| unsafe {
            let lib = LoadLibraryExW(w!("wlanapi.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
            Some(Api {
                open: std::mem::transmute(GetProcAddress(lib, s!("WlanOpenHandle"))?),
                enum_interfaces: std::mem::transmute(GetProcAddress(lib, s!("WlanEnumInterfaces"))?),
                query: std::mem::transmute(GetProcAddress(lib, s!("WlanQueryInterface"))?),
                free: std::mem::transmute(GetProcAddress(lib, s!("WlanFreeMemory"))?),
                close: std::mem::transmute(GetProcAddress(lib, s!("WlanCloseHandle"))?),
            })
        })
        .as_ref()
    }
}

#[cfg(windows)]
pub fn wifi_info(name: &str) -> Option<WifiInfo> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::WiFi::*;
    use windows::core::GUID;
    let api = wlanapi::get()?;
    let want = name.trim_matches(|c| c == '{' || c == '}').to_uppercase();
    let null = std::ptr::null();
    unsafe {
        let mut ver = 0u32;
        let mut h = HANDLE::default();
        if (api.open)(2, null, &mut ver, &mut h) != 0 {
            return None;
        }
        let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        let mut out = None;
        if (api.enum_interfaces)(h, null, &mut list) == 0 && !list.is_null() {
            let l = &*list;
            let items = std::slice::from_raw_parts(l.InterfaceInfo.as_ptr(), l.dwNumberOfItems as usize);
            for it in items {
                let g: GUID = it.InterfaceGuid;
                if format!("{g:?}").to_uppercase() != want {
                    continue;
                }
                let mut info = WifiInfo::default();
                let mut size = 0u32;
                let mut data: *mut core::ffi::c_void = std::ptr::null_mut();
                if (api.query)(h, &g, wlan_intf_opcode_current_connection, null, &mut size, &mut data, std::ptr::null_mut()) == 0 && !data.is_null() {
                    let a = &(*(data as *const WLAN_CONNECTION_ATTRIBUTES)).wlanAssociationAttributes;
                    let ssid = &a.dot11Ssid;
                    info.ssid = Some(String::from_utf8_lossy(&ssid.ucSSID[..ssid.uSSIDLength.min(32) as usize]).into_owned());
                    info.bssid = Some(a.dot11Bssid.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":"));
                    info.quality_pct = Some(a.wlanSignalQuality);
                    info.tx_rate_mbps = Some(a.ulTxRate as f64 / 1000.0).filter(|r| *r > 0.0);
                    (api.free)(data);
                }
                for (op, target) in [(wlan_intf_opcode_rssi, 0u8), (wlan_intf_opcode_channel_number, 1u8)] {
                    let mut data: *mut core::ffi::c_void = std::ptr::null_mut();
                    if (api.query)(h, &g, op, null, &mut size, &mut data, std::ptr::null_mut()) == 0 && !data.is_null() {
                        if target == 0 {
                            info.rssi_dbm = Some(*(data as *const i32)).filter(|r| *r < 0);
                        } else {
                            let c = *(data as *const u32);
                            info.channel = Some(c).filter(|c| *c > 0);
                            info.band = info.channel.map(|c| if c <= 14 { "2,4 GHz".into() } else { "5 GHz".into() });
                        }
                        (api.free)(data);
                    }
                }
                out = Some(info);
            }
            (api.free)(list as *const core::ffi::c_void);
        }
        (api.close)(h, null);
        out
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn wifi_info(_name: &str) -> Option<WifiInfo> {
    None
}

/// macOS: Ortungsdienste-Freigabe anfragen, ohne die macOS die SSID verschweigt (einmal beim Start,
/// auf dem Haupt-Thread). Der Manager muss am Leben bleiben, daher wird er absichtlich nie freigegeben.
#[cfg(target_os = "macos")]
pub fn request_location_permission() {
    use objc2_core_location::CLLocationManager;
    unsafe {
        let m = CLLocationManager::new();
        m.requestWhenInUseAuthorization();
        std::mem::forget(m);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_location_permission() {}

// ----------------------------------------------------------------- Strom/Ruhezustand

pub fn on_battery() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("pmset")
            .args(["-g", "batt"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("Battery Power"))
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut s = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut s).is_ok() && s.ACLineStatus == 0 }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        false
    }
}

/// Verhindert Ruhezustand/Energiesparen, solange der Wert lebt.
pub struct KeepAwake {
    #[cfg(target_os = "macos")]
    child: Option<std::process::Child>,
    #[cfg(windows)]
    stop: Option<std::sync::mpsc::Sender<()>>,
}

impl KeepAwake {
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            let child = std::process::Command::new("caffeinate").arg("-i").spawn().ok();
            KeepAwake { child }
        }
        #[cfg(windows)]
        {
            use windows::Win32::System::Power::{ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState};
            // Der Zustand gilt pro Thread -> eigener Thread, der bis zum Ende wartet
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            std::thread::spawn(move || unsafe {
                SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
                let _ = rx.recv();
                SetThreadExecutionState(ES_CONTINUOUS);
            });
            KeepAwake { stop: Some(tx) }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            KeepAwake {}
        }
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        #[cfg(windows)]
        drop(self.stop.take());
    }
}
