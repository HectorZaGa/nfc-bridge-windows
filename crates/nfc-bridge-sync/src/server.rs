//! # Driver IPC Client (Named Pipe)
//!
//! `nfc-bridge-sync` NO abre puertos de red hacia el celular ni emula PC/SC por su cuenta.
//! Toda la conectividad (TCP, UDP Auto-Discovery, USB AOA, Bluetooth, Wi-Fi Direct y Cloud Relay)
//! y la emulación PC/SC residen exclusivamente en `nfc-bridge-driver`.
//!
//! Este módulo mantiene conexión bidireccional con el driver en `\\.\pipe\nfc_bridge_driver`,
//! actualiza `DesktopLiveState` y permite que la UI local solicite QRs efímeros o envíe órdenes.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tracing::{info, warn};

use crate::driver_manager::DRIVER_NAMED_PIPE;

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

#[derive(Debug, Clone, Serialize, Default)]
pub struct IssuedQrData {
    pub method: String,
    pub pair_token: String,
    pub expires_in_secs: u64,
    pub uri: String,
    pub share_link: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingMobileSwitchState {
    pub request_id: String,
    pub device_id: Option<String>,
    pub device_name: String,
    pub mobile_id: Option<String>,
    pub device_os: Option<String>,
    pub transport: String,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Clone)]
pub struct DriverIpcClient {
    state: Arc<RwLock<DesktopLiveState>>,
    cmd_tx: Arc<Mutex<Option<mpsc::Sender<IpcCommand>>>>,
    pending_qr_waiter: Arc<Mutex<Option<oneshot::Sender<IssuedQrData>>>>,
}

impl DriverIpcClient {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(DesktopLiveState::default())),
            cmd_tx: Arc::new(Mutex::new(None)),
            pending_qr_waiter: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_live_state(&self) -> DesktopLiveState {
        let accent = read_windows_accent_hex();
        let mut st = self.state.write().await;
        st.windows_accent_hex = accent;
        st.clone()
    }

    pub async fn update_ui_settings(&self, settings: SyncUiSettings) {
        let clean = SyncUiSettings {
            theme_mode: match settings.theme_mode.as_str() {
                "light" | "system" | "dark" => settings.theme_mode,
                _ => "dark".to_string(),
            },
            use_windows_accent: settings.use_windows_accent,
            custom_accent_hex: normalize_hex_color(&settings.custom_accent_hex),
            auto_update_app: settings.auto_update_app,
            auto_update_driver: settings.auto_update_driver,
            language: match settings.language.as_str() {
                "en" | "es" | "system" => settings.language,
                _ => "system".to_string(),
            },
        };
        save_ui_settings(&clean);
        let mut st = self.state.write().await;
        st.ui_settings = clean;
    }

    pub async fn set_product_meta(&self, version: &str, build_id: &str) {
        let mut st = self.state.write().await;
        st.sync_version = version.to_string();
        st.sync_build_id = build_id.to_string();
    }

    pub async fn clear_pending_mobile_switch(&self, request_id: Option<&str>) {
        let mut st = self.state.write().await;
        if request_id.is_none()
            || st
                .pending_mobile_switch
                .as_ref()
                .is_some_and(|p| Some(p.request_id.as_str()) == request_id)
        {
            st.pending_mobile_switch = None;
        }
    }

    pub async fn clear_invalid_pairing_notice(&self) {
        let mut st = self.state.write().await;
        st.pending_invalid_pairing = None;
    }

    pub async fn select_saved_profile(&self, device_id: &str) {
        let mut st = self.state.write().await;
        let target_dev = st
            .snapshot
            .trusted_devices
            .iter()
            .find(|d| d.device_id == device_id)
            .cloned();
        let same_connected_phone = st.connected
            && target_dev.as_ref().is_some_and(|dev| match (&st.mobile_id, &dev.mobile_id) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => st.device_name.as_deref() == Some(dev.device_name.as_str()),
            });
        if same_connected_phone {
            return;
        }
        st.selected_device_id = Some(device_id.to_string());
        if let Some(dev) = target_dev {
            st.last_device_name = Some(dev.device_name);
            st.last_transport = dev.transport;
            st.last_mobile_id = dev.mobile_id;
            st.last_device_os = dev.device_os;
            st.last_endpoint = dev.endpoint;
        }
    }

    pub async fn remove_saved_profile(&self, device_id: &str) {
        let mut st = self.state.write().await;
        st.snapshot
            .trusted_devices
            .retain(|d| d.device_id != device_id);
        if st.active_device_id.as_deref() == Some(device_id) {
            st.connected = false;
            st.device_name = None;
            st.endpoint = None;
            st.device_os = None;
            st.active_device_id = None;
            st.paused = false;
            st.tag_present = false;
            st.tag_uid = None;
        }
        Self::reconcile_saved_selection(&mut st, Some(device_id));
    }

    pub async fn rename_saved_profile(&self, device_id: &str, new_name: &str) {
        let mut st = self.state.write().await;
        if let Some(dev) = st
            .snapshot
            .trusted_devices
            .iter_mut()
            .find(|d| d.device_id == device_id)
        {
            dev.device_name = new_name.to_string();
        }
        if st.connected && st.active_device_id.as_deref() == Some(device_id) {
            st.device_name = Some(new_name.to_string());
        }
        if st.selected_device_id.as_deref() == Some(device_id)
            || st.active_device_id.as_deref() == Some(device_id)
        {
            st.last_device_name = Some(new_name.to_string());
        }
    }

    fn reconcile_saved_selection(st: &mut DesktopLiveState, removed_id: Option<&str>) {
        let still_exists = st
            .selected_device_id
            .as_deref()
            .map(|sid| st.snapshot.trusted_devices.iter().any(|d| d.device_id == sid))
            .unwrap_or(false);
        if !still_exists || removed_id.is_some_and(|rid| st.selected_device_id.as_deref() == Some(rid))
        {
            st.selected_device_id = st
                .snapshot
                .trusted_devices
                .first()
                .map(|d| d.device_id.clone());
        }
        if !st.connected {
            if let Some(first_or_sel) = st
                .selected_device_id
                .as_deref()
                .and_then(|sid| st.snapshot.trusted_devices.iter().find(|d| d.device_id == sid))
                .or_else(|| st.snapshot.trusted_devices.first())
                .cloned()
            {
                st.last_device_name = Some(first_or_sel.device_name);
                st.last_transport = first_or_sel.transport;
                st.last_mobile_id = first_or_sel.mobile_id;
                st.last_device_os = first_or_sel.device_os;
                st.last_endpoint = first_or_sel.endpoint;
            } else {
                st.selected_device_id = None;
                st.last_device_name = None;
                st.last_transport = None;
                st.last_mobile_id = None;
                st.last_device_os = None;
                st.last_endpoint = None;
            }
        }
    }

    pub async fn send_command(&self, cmd: IpcCommand) -> Result<()> {
        let tx = {
            let guard = self.cmd_tx.lock().await;
            guard.clone()
        };
        let Some(sender) = tx else {
            anyhow::bail!("Driver desconectado del canal IPC");
        };
        sender
            .send(cmd)
            .await
            .map_err(|_| anyhow::anyhow!("No se pudo enviar comando al driver"))
    }

    pub async fn request_pairing_qr(&self, method: &str) -> Result<IssuedQrData> {
        let (tx, rx) = oneshot::channel();
        {
            let mut waiter = self.pending_qr_waiter.lock().await;
            *waiter = Some(tx);
        }

        if let Err(e) = self
            .send_command(IpcCommand::IssuePairingToken {
                method: method.to_string(),
            })
            .await
        {
            let mut waiter = self.pending_qr_waiter.lock().await;
            *waiter = None;
            return Err(e);
        }

        tokio::time::timeout(Duration::from_secs(3), rx)
            .await
            .map_err(|_| anyhow::anyhow!("Timeout esperando QR del driver"))?
            .map_err(|_| anyhow::anyhow!("Canal QR cerrado"))
    }

    /// Mantiene conexión IPC persistente con `nfc-bridge-driver`.
    pub async fn run_forever(&self) -> Result<()> {
        let mut logged_waiting = false;

        loop {
            match self.connect_and_stream().await {
                Ok(()) => {
                    logged_waiting = false;
                    warn!("Conexión IPC con el driver cerrada. Reintentando enlace...");
                }
                Err(_) => {
                    if !logged_waiting {
                        logged_waiting = true;
                        warn!(
                            "Esperando a NFC Bridge Driver ({})... Sin el driver activo no es posible conectar por TCP, USB AOA, Bluetooth, Wi-Fi Direct ni Relay.",
                            DRIVER_NAMED_PIPE
                        );
                    }
                }
            }
            {
                let mut guard = self.cmd_tx.lock().await;
                *guard = None;
            }
            {
                let mut st = self.state.write().await;
                st.driver_online = false;
                st.connected = false;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn connect_and_stream(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use tokio::net::windows::named_pipe::ClientOptions;

            let client = ClientOptions::new().open(DRIVER_NAMED_PIPE)?;
            info!("Enlazado con NFC Bridge Driver vía IPC ({})", DRIVER_NAMED_PIPE);

            {
                let mut st = self.state.write().await;
                st.driver_online = true;
            }

            let (tx, mut rx) = mpsc::channel::<IpcCommand>(32);
            {
                let mut guard = self.cmd_tx.lock().await;
                *guard = Some(tx.clone());
            }

            let (reader, mut writer) = tokio::io::split(client);
            let mut lines = BufReader::new(reader).lines();

            // Solicitar snapshot inicial de transportes
            let _ = tx.send(IpcCommand::GetTransportSnapshot).await;

            loop {
                tokio::select! {
                    Some(cmd) = rx.recv() => {
                        if let Ok(json) = serde_json::to_string(&cmd) {
                            if writer.write_all(format!("{}\n", json).as_bytes()).await.is_err() {
                                break;
                            }
                            let _ = writer.flush().await;
                        }
                    }
                    line_res = lines.next_line() => {
                        match line_res {
                            Ok(Some(line)) => {
                                if let Ok(event) = serde_json::from_str::<IpcEvent>(&line) {
                                    self.handle_driver_event(event).await;
                                }
                            }
                            _ => break,
                        }
                    }
                }
            }

            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Named Pipe IPC solo soportado en Windows");
        }
    }

    async fn handle_driver_event(&self, event: IpcEvent) {
        match event {
            IpcEvent::Status {
                connected,
                device_name,
                latency_ms,
                paused,
                tag_present,
                tag_uid,
                transport,
                mobile_id,
                device_os,
                active_device_id,
            } => {
                {
                    let mut st = self.state.write().await;
                    st.connected = connected;
                    st.device_name = device_name.clone();
                    st.transport = transport.clone();
                    st.mobile_id = mobile_id.clone();
                    st.device_os = device_os.clone();
                    st.active_device_id = active_device_id.clone();
                    if connected {
                        if let Some(ref aid) = active_device_id {
                            st.selected_device_id = Some(aid.clone());
                        }
                    }
                    if let Some(ref name) = device_name {
                        st.last_device_name = Some(name.clone());
                    }
                    if let Some(ref tr) = transport {
                        st.last_transport = Some(tr.clone());
                    }
                    if let Some(ref mid) = mobile_id {
                        st.last_mobile_id = Some(mid.clone());
                    }
                    if let Some(ref os) = device_os {
                        st.last_device_os = Some(os.clone());
                    }
                    st.latency_ms = latency_ms;
                    st.paused = paused;
                    st.tag_present = tag_present;
                    st.tag_uid = tag_uid.clone();
                }
                if connected {
                    info!(
                        "[Driver] Estado: Conectado a '{}' ({} ms) | Pausado={} | Tag={:?}",
                        device_name.unwrap_or_else(|| "Android".to_string()),
                        latency_ms,
                        paused,
                        if tag_present { tag_uid } else { None }
                    );
                } else {
                    info!("[Driver] Listo y en escucha (sin teléfono conectado aún).");
                }
            }
            IpcEvent::WifiDirectStatus {
                available,
                active,
                pc_name,
                ssid,
                passphrase,
                ip,
                port,
                reason,
            } => {
                {
                    let mut st = self.state.write().await;
                    st.snapshot.pc_name = pc_name.clone();
                    st.snapshot.wfd_available = available && active;
                    st.snapshot.wfd_ssid = ssid.clone();
                    st.snapshot.wfd_passphrase = passphrase.clone();
                    st.snapshot.wfd_ip = ip.clone();
                }
                if available && active {
                    if let (Some(ssid), Some(pass)) = (ssid.as_deref(), passphrase.as_deref()) {
                        let target_ip = ip.as_deref().unwrap_or("192.168.137.1");
                        let qr_uri =
                            build_wifi_direct_qr_uri(target_ip, port, &pc_name, ssid, pass);
                        info!(
                            "[Driver] Wi-Fi Direct activo | SSID='{}' | Clave='{}' | Enlace QR: {}",
                            ssid, pass, qr_uri
                        );
                    }
                } else if let Some(msg) = reason {
                    info!("[Driver] Wi-Fi Direct no disponible: {}", msg);
                }
            }
            IpcEvent::TransportSnapshot {
                pc_name,
                tcp_ip,
                tcp_port,
                bt_available,
                bt_mac,
                bt_name,
                relay_url,
                relay_channel,
                relay_connected,
                wfd_available,
                wfd_ssid,
                wfd_passphrase,
                wfd_ip,
                tcp_enabled,
                wfd_enabled,
                bt_enabled,
                usb_enabled,
                usb_auto_connect,
                usb_connected,
                driver_version,
                driver_build_id,
                tcp_ip_override,
                auto_accept_mobile_switch,
                trusted_devices,
            } => {
                let mut st = self.state.write().await;
                st.snapshot = TransportSnapshotData {
                    pc_name,
                    tcp_ip,
                    tcp_port,
                    bt_available,
                    bt_mac,
                    bt_name,
                    relay_url,
                    relay_channel,
                    relay_connected,
                    wfd_available,
                    wfd_ssid,
                    wfd_passphrase,
                    wfd_ip,
                    tcp_enabled,
                    wfd_enabled,
                    bt_enabled,
                    usb_enabled,
                    usb_auto_connect,
                    usb_connected,
                    driver_version,
                    driver_build_id,
                    tcp_ip_override,
                    auto_accept_mobile_switch,
                    trusted_devices,
                };
                Self::reconcile_saved_selection(&mut st, None);
            }
            IpcEvent::PendingMobileSwitch {
                request_id,
                device_id,
                device_name,
                mobile_id,
                device_os,
                transport,
            } => {
                let mut st = self.state.write().await;
                st.pending_mobile_switch = Some(PendingMobileSwitchState {
                    request_id,
                    device_id,
                    device_name,
                    mobile_id,
                    device_os,
                    transport,
                });
            }
            IpcEvent::PendingMobileSwitchCleared { request_id } => {
                let mut st = self.state.write().await;
                if st
                    .pending_mobile_switch
                    .as_ref()
                    .is_some_and(|p| p.request_id == request_id)
                {
                    st.pending_mobile_switch = None;
                }
            }
            IpcEvent::PairingInvalidNotice {
                device_id,
                device_name,
                mobile_id,
                transport,
                reason,
            } => {
                let mut st = self.state.write().await;
                st.pending_invalid_pairing = Some(InvalidPairingNoticeState {
                    device_id,
                    device_name,
                    mobile_id,
                    transport,
                    reason,
                });
            }
            IpcEvent::PairingTokenIssued {
                method,
                pair_token,
                expires_in_secs,
                uri,
                share_link,
            } => {
                let data = IssuedQrData {
                    method,
                    pair_token,
                    expires_in_secs,
                    uri,
                    share_link,
                };
                if let Some(waiter) = self.pending_qr_waiter.lock().await.take() {
                    let _ = waiter.send(data);
                }
            }
            IpcEvent::DeviceConnected {
                device_name,
                endpoint,
            } => {
                {
                    let mut st = self.state.write().await;
                    st.connected = true;
                    st.device_name = Some(device_name.clone());
                    st.endpoint = Some(endpoint.clone());
                    st.last_device_name = Some(device_name.clone());
                    st.last_endpoint = Some(endpoint.clone());
                }
                info!("[Driver] Dispositivo Android conectado: '{}' vía {}", device_name, endpoint);
            }
            IpcEvent::DeviceDisconnected => {
                {
                    let mut st = self.state.write().await;
                    st.connected = false;
                    st.device_name = None;
                    st.endpoint = None;
                    st.device_os = None;
                    st.paused = false;
                    st.tag_present = false;
                    st.tag_uid = None;
                }
                info!("[Driver] Dispositivo móvil desconectado.");
            }
            IpcEvent::LatencyUpdated { latency_ms } => {
                let mut st = self.state.write().await;
                st.latency_ms = latency_ms;
            }
            IpcEvent::TagAttached {
                uid,
                technologies,
                is_ntag215,
            } => {
                {
                    let mut st = self.state.write().await;
                    st.tag_present = true;
                    st.tag_uid = Some(uid.clone());
                }
                let tag_type = if is_ntag215 || technologies.iter().any(|t| t == "MifareUltralight") {
                    "NTAG / Ultralight (ISO 14443-3A)".to_string()
                } else if technologies.iter().any(|t| t == "IsoDep") {
                    "ISO-DEP / Tarjeta inteligente (ISO 14443-4)".to_string()
                } else if technologies.iter().any(|t| t == "NfcA") {
                    "ISO 14443-3A (NfcA)".to_string()
                } else if technologies.is_empty() {
                    "Etiqueta NFC".to_string()
                } else {
                    technologies.join(", ")
                };
                info!("[Driver] Etiqueta NFC detectada: {}", tag_type);
            }
            IpcEvent::TagDetached => {
                {
                    let mut st = self.state.write().await;
                    st.tag_present = false;
                    st.tag_uid = None;
                }
                info!("[Driver] Etiqueta NFC retirada del campo.");
            }
            IpcEvent::Error { message } => {
                warn!("[Driver Error] {}", message);
            }
        }
    }
}

/// Construye el URI estándar `nfcbridge://` con parámetros de Wi-Fi Direct (SoftAP + TCP) para QR.
pub fn build_wifi_direct_qr_uri(
    ip: &str,
    port: u16,
    pc_name: &str,
    ssid: &str,
    passphrase: &str,
) -> String {
    format!(
        "nfcbridge://{}:{}?name={}&transport=wifi_direct&ssid={}&pass={}",
        ip,
        port,
        percent_encode_query(pc_name),
        percent_encode_query(ssid),
        percent_encode_query(passphrase),
    )
}

fn percent_encode_query(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

fn normalize_hex_color(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches('#');
    if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{}", trimmed.to_ascii_uppercase())
    } else {
        "#3B82F6".to_string()
    }
}

fn ui_settings_path() -> std::path::PathBuf {
    let base = std::env::var("APPDATA")
        .or_else(|_| std::env::var("LOCALAPPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(base)
        .join("NFC Bridge")
        .join("sync_ui_settings.json")
}

pub fn load_ui_settings() -> SyncUiSettings {
    std::fs::read_to_string(ui_settings_path())
        .ok()
        .and_then(|s| serde_json::from_str::<SyncUiSettings>(&s).ok())
        .unwrap_or_default()
}

fn save_ui_settings(cfg: &SyncUiSettings) {
    let path = ui_settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(path, json);
    }
}

pub fn read_windows_accent_hex() -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(out) = std::process::Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\DWM",
                "/v",
                "AccentColor",
            ])
            .creation_flags(0x0800_0000)
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(token) = text
                    .split_whitespace()
                    .find(|t| t.starts_with("0x") || t.starts_with("0X"))
                {
                    if let Ok(abgr) = u32::from_str_radix(&token[2..], 16) {
                        let r = (abgr & 0xFF) as u8;
                        let g = ((abgr >> 8) & 0xFF) as u8;
                        let b = ((abgr >> 16) & 0xFF) as u8;
                        return format!("#{:02X}{:02X}{:02X}", r, g, b);
                    }
                }
            }
        }
    }
    "#3B82F6".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wifi_direct_qr_uri_encoding() {
        let uri = build_wifi_direct_qr_uri(
            "192.168.137.1",
            45362,
            "HECTOR MASTER",
            "DIRECT-NB-HECTOR",
            "Abc123XY",
        );
        assert_eq!(
            uri,
            "nfcbridge://192.168.137.1:45362?name=HECTOR%20MASTER&transport=wifi_direct&ssid=DIRECT-NB-HECTOR&pass=Abc123XY"
        );
    }

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
