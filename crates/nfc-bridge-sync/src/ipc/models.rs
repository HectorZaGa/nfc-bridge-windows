use crate::config::ui_settings_path;
use crate::platform::read_windows_accent_hex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedDeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub created_unix: u64,
    #[serde(default)]
    pub mobile_id: Option<String>,
    #[serde(default)]
    pub device_os: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "command", content = "params")]
pub enum IpcCommand {
    GetStatus,
    GetWifiDirectInfo,
    GetTransportSnapshot,
    IssuePairingToken { method: String },
    SetRelayUrl { url: Option<String> },
    SetPaused { paused: bool },
    SetUsbPriority { enabled: bool },
    SetUsbEnabled { enabled: bool },
    SetTcpEnabled { enabled: bool },
    SetWifiDirectEnabled { enabled: bool },
    SetBluetoothEnabled { enabled: bool },
    DisconnectDevice,
    RevokeTrustedDevice { device_id: String },
    RenameTrustedDevice { device_id: String, name: String },
    SelectTrustedDevice { device_id: String },
    SetTcpNetworkConfig {
        tcp_ip: Option<String>,
        tcp_port: Option<u16>,
        reset: bool,
    },
    SetAutoAcceptMobileSwitch { enabled: bool },
    RespondMobileSwitch { request_id: String, accept: bool },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum IpcEvent {
    Status {
        connected: bool,
        device_name: Option<String>,
        latency_ms: u64,
        paused: bool,
        tag_present: bool,
        tag_uid: Option<String>,
        #[serde(default)]
        transport: Option<String>,
        #[serde(default)]
        mobile_id: Option<String>,
        #[serde(default)]
        device_os: Option<String>,
        #[serde(default)]
        active_device_id: Option<String>,
    },
    WifiDirectStatus {
        available: bool,
        active: bool,
        pc_name: String,
        ssid: Option<String>,
        passphrase: Option<String>,
        ip: Option<String>,
        port: u16,
        reason: Option<String>,
    },
    TransportSnapshot {
        pc_name: String,
        tcp_ip: Option<String>,
        tcp_port: u16,
        bt_available: bool,
        bt_mac: Option<String>,
        bt_name: Option<String>,
        relay_url: Option<String>,
        relay_channel: String,
        relay_connected: bool,
        wfd_available: bool,
        wfd_ssid: Option<String>,
        wfd_passphrase: Option<String>,
        wfd_ip: Option<String>,
        #[serde(default = "default_true")]
        tcp_enabled: bool,
        #[serde(default)]
        wfd_enabled: bool,
        #[serde(default = "default_true")]
        bt_enabled: bool,
        #[serde(default = "default_true")]
        usb_enabled: bool,
        #[serde(default = "default_true")]
        usb_auto_connect: bool,
        #[serde(default)]
        usb_connected: bool,
        #[serde(default)]
        driver_version: Option<String>,
        #[serde(default)]
        driver_build_id: Option<String>,
        #[serde(default)]
        tcp_ip_override: Option<String>,
        #[serde(default)]
        auto_accept_mobile_switch: bool,
        #[serde(default)]
        trusted_devices: Vec<TrustedDeviceInfo>,
    },
    PendingMobileSwitch {
        request_id: String,
        device_id: Option<String>,
        device_name: String,
        mobile_id: Option<String>,
        device_os: Option<String>,
        transport: String,
    },
    PendingMobileSwitchCleared {
        request_id: String,
    },
    PairingInvalidNotice {
        device_id: Option<String>,
        device_name: String,
        mobile_id: Option<String>,
        transport: Option<String>,
        reason: String,
    },
    PairingTokenIssued {
        method: String,
        pair_token: String,
        expires_in_secs: u64,
        uri: String,
        share_link: Option<String>,
    },
    DeviceConnected {
        device_name: String,
        endpoint: String,
    },
    DeviceDisconnected,
    LatencyUpdated {
        latency_ms: u64,
    },
    TagAttached {
        uid: String,
        technologies: Vec<String>,
        is_ntag215: bool,
    },
    TagDetached,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct IssuedQrData {
    pub method: String,
    pub pair_token: String,
    pub expires_in_secs: u64,
    pub uri: String,
    pub share_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TransportSnapshotData {
    pub pc_name: String,
    pub tcp_ip: Option<String>,
    pub tcp_port: u16,
    pub bt_available: bool,
    pub bt_mac: Option<String>,
    pub bt_name: Option<String>,
    pub relay_url: Option<String>,
    pub relay_channel: String,
    pub relay_connected: bool,
    pub wfd_available: bool,
    pub wfd_ssid: Option<String>,
    pub wfd_passphrase: Option<String>,
    pub wfd_ip: Option<String>,
    pub tcp_enabled: bool,
    pub wfd_enabled: bool,
    pub bt_enabled: bool,
    pub usb_enabled: bool,
    pub usb_auto_connect: bool,
    pub usb_connected: bool,
    pub driver_version: Option<String>,
    pub driver_build_id: Option<String>,
    pub tcp_ip_override: Option<String>,
    pub auto_accept_mobile_switch: bool,
    pub trusted_devices: Vec<TrustedDeviceInfo>,
}

fn default_language() -> String {
    "system".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncUiSettings {
    pub theme_mode: String,
    pub use_windows_accent: bool,
    pub custom_accent_hex: String,
    #[serde(default)]
    pub auto_update_app: bool,
    #[serde(default)]
    pub auto_update_driver: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub settings_sidebar_compact: bool,
}

impl Default for SyncUiSettings {
    fn default() -> Self {
        Self {
            theme_mode: "dark".to_string(),
            use_windows_accent: false,
            custom_accent_hex: "#3B82F6".to_string(),
            auto_update_app: false,
            auto_update_driver: false,
            language: default_language(),
            settings_sidebar_compact: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PendingMobileSwitchState {
    pub request_id: String,
    pub device_id: Option<String>,
    pub device_name: String,
    pub mobile_id: Option<String>,
    pub device_os: Option<String>,
    pub transport: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InvalidPairingNoticeState {
    pub device_id: Option<String>,
    pub device_name: String,
    pub mobile_id: Option<String>,
    pub transport: Option<String>,
    pub reason: String,
}

impl Default for TransportSnapshotData {
    fn default() -> Self {
        Self {
            pc_name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "NFC Bridge PC".to_string()),
            tcp_ip: None,
            tcp_port: 45362,
            bt_available: false,
            bt_mac: None,
            bt_name: None,
            relay_url: None,
            relay_channel: String::new(),
            relay_connected: false,
            wfd_available: false,
            wfd_ssid: None,
            wfd_passphrase: None,
            wfd_ip: None,
            tcp_enabled: true,
            wfd_enabled: false,
            bt_enabled: true,
            usb_enabled: true,
            usb_auto_connect: true,
            usb_connected: false,
            driver_version: None,
            driver_build_id: None,
            tcp_ip_override: None,
            auto_accept_mobile_switch: false,
            trusted_devices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DesktopLiveState {
    pub sync_version: String,
    pub sync_build_id: String,
    pub driver_online: bool,
    pub connected: bool,
    pub device_name: Option<String>,
    pub endpoint: Option<String>,
    pub transport: Option<String>,
    pub mobile_id: Option<String>,
    pub device_os: Option<String>,
    pub active_device_id: Option<String>,
    pub selected_device_id: Option<String>,
    pub last_device_name: Option<String>,
    pub last_endpoint: Option<String>,
    pub last_transport: Option<String>,
    pub last_mobile_id: Option<String>,
    pub last_device_os: Option<String>,
    pub latency_ms: u64,
    pub paused: bool,
    pub tag_present: bool,
    pub tag_uid: Option<String>,
    pub ui_settings: SyncUiSettings,
    pub windows_accent_hex: String,
    pub pending_mobile_switch: Option<PendingMobileSwitchState>,
    pub pending_invalid_pairing: Option<InvalidPairingNoticeState>,
    pub snapshot: TransportSnapshotData,
}

impl Default for DesktopLiveState {
    fn default() -> Self {
        Self {
            sync_version: env!("CARGO_PKG_VERSION").to_string(),
            sync_build_id: env!("NFC_BRIDGE_BUILD_ID").to_string(),
            driver_online: false,
            connected: false,
            device_name: None,
            endpoint: None,
            transport: None,
            mobile_id: None,
            device_os: None,
            active_device_id: None,
            selected_device_id: None,
            last_device_name: None,
            last_endpoint: None,
            last_transport: None,
            last_mobile_id: None,
            last_device_os: None,
            latency_ms: 0,
            paused: false,
            tag_present: false,
            tag_uid: None,
            ui_settings: load_ui_settings(),
            windows_accent_hex: read_windows_accent_hex(),
            pending_mobile_switch: None,
            pending_invalid_pairing: None,
            snapshot: TransportSnapshotData::default(),
        }
    }
}

pub fn load_ui_settings() -> SyncUiSettings {
    std::fs::read_to_string(ui_settings_path())
        .ok()
        .and_then(|s| serde_json::from_str::<SyncUiSettings>(&s).ok())
        .unwrap_or_default()
}

pub fn save_ui_settings(cfg: &SyncUiSettings) {
    let path = ui_settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_wifi_direct_status_event() {
        let raw = r#"{"event":"WifiDirectStatus","data":{"available":true,"active":true,"pc_name":"PC","ssid":"DIRECT-NB-PC","passphrase":"Pass1234","ip":"192.168.137.1","port":45362,"reason":null}}"#;
        let parsed: IpcEvent = serde_json::from_str(raw).expect("valid WifiDirectStatus JSON");
        match parsed {
            IpcEvent::WifiDirectStatus { available, ssid, .. } => {
                assert!(available);
                assert_eq!(ssid.as_deref(), Some("DIRECT-NB-PC"));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }
}
