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
        trusted_devices: Vec<TrustedDeviceInfo>,
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
    pub trusted_devices: Vec<TrustedDeviceInfo>,
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
    pub active_device_id: Option<String>,
    pub last_device_name: Option<String>,
    pub last_endpoint: Option<String>,
    pub last_transport: Option<String>,
    pub last_mobile_id: Option<String>,
    pub latency_ms: u64,
    pub paused: bool,
    pub tag_present: bool,
    pub tag_uid: Option<String>,
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
            active_device_id: None,
            last_device_name: None,
            last_endpoint: None,
            last_transport: None,
            last_mobile_id: None,
            latency_ms: 0,
            paused: false,
            tag_present: false,
            tag_uid: None,
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
        self.state.read().await.clone()
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
                active_device_id,
            } => {
                {
                    let mut st = self.state.write().await;
                    st.connected = connected;
                    st.device_name = device_name.clone();
                    st.transport = transport.clone();
                    st.mobile_id = mobile_id.clone();
                    st.active_device_id = active_device_id;
                    if let Some(ref name) = device_name {
                        st.last_device_name = Some(name.clone());
                    }
                    if let Some(ref tr) = transport {
                        st.last_transport = Some(tr.clone());
                    }
                    if let Some(ref mid) = mobile_id {
                        st.last_mobile_id = Some(mid.clone());
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
                    trusted_devices,
                };
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
                    st.paused = false;
                    st.tag_present = false;
                    st.tag_uid = None;
                }
                info!("[Driver] Dispositivo Android desconectado.");
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
                info!(
                    "[Driver] Etiqueta NFC detectada: UID={} | Techs={:?} | NTAG215={}",
                    uid, technologies, is_ntag215
                );
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
