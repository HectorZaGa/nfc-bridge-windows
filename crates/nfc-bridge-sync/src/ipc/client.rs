use super::models::{
    save_ui_settings, DesktopLiveState, InvalidPairingNoticeState, IpcCommand, IpcEvent,
    IssuedQrData, PendingMobileSwitchState, SyncUiSettings, TransportSnapshotData,
};
use super::uri::{build_wifi_direct_qr_uri, normalize_hex_color};
use crate::driver_manager::DRIVER_NAMED_PIPE;
use crate::platform::read_windows_accent_hex;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, Mutex, Notify, RwLock};
use tracing::{info, warn};

#[derive(Clone)]
pub struct DriverIpcClient {
    state: Arc<RwLock<DesktopLiveState>>,
    cmd_tx: Arc<Mutex<Option<mpsc::Sender<IpcCommand>>>>,
    pending_qr_waiter: Arc<Mutex<Option<oneshot::Sender<IssuedQrData>>>>,
    state_notify: Arc<Notify>,
}

impl Default for DriverIpcClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverIpcClient {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(DesktopLiveState::default())),
            cmd_tx: Arc::new(Mutex::new(None)),
            pending_qr_waiter: Arc::new(Mutex::new(None)),
            state_notify: Arc::new(Notify::new()),
        }
    }

    pub fn notifier(&self) -> Arc<Notify> {
        self.state_notify.clone()
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
            settings_sidebar_compact: settings.settings_sidebar_compact,
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
            && target_dev
                .as_ref()
                .is_some_and(|dev| match (&st.mobile_id, &dev.mobile_id) {
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
        if !still_exists
            || removed_id.is_some_and(|rid| st.selected_device_id.as_deref() == Some(rid))
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
                                    self.state_notify.notify_one();
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
                info!(
                    "[Driver] Dispositivo Android conectado: '{}' vía {}",
                    device_name, endpoint
                );
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
                let tag_type = if is_ntag215 || technologies.iter().any(|t| t == "MifareUltralight")
                {
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
