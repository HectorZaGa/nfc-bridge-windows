//! Servidor HTTP local ligero (127.0.0.1:45363) para servir la UI de escritorio Stitch
//! (`EmbeddedUi::INDEX_HTML`), generar códigos QR SVG offline de alta precisión y puentear
//! comandos en tiempo real hacia `nfc-bridge-driver` vía IPC Named Pipe.

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

use crate::config::EmbeddedUi;
use crate::server::{DriverIpcClient, IpcCommand};
use crate::updater::DesktopUpdater;

pub const DEFAULT_UI_HTTP_PORT: u16 = 45363;

#[derive(Deserialize)]
struct QrRequest {
    method: String,
}

#[derive(Deserialize)]
struct CommandRequest {
    command: String,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    tcp_ip: Option<String>,
    #[serde(default)]
    tcp_port: Option<u16>,
    #[serde(default)]
    reset: Option<bool>,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    accept: Option<bool>,
    #[serde(default)]
    delete_profile: Option<bool>,
    #[serde(default)]
    theme_mode: Option<String>,
    #[serde(default)]
    use_windows_accent: Option<bool>,
    #[serde(default)]
    custom_accent_hex: Option<String>,
    #[serde(default)]
    auto_update_app: Option<bool>,
    #[serde(default)]
    auto_update_driver: Option<bool>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Deserialize)]
struct RelayRequest {
    url: Option<String>,
}

pub struct DesktopUiServer {
    ipc: DriverIpcClient,
    updater: DesktopUpdater,
    port: u16,
}

impl DesktopUiServer {
    pub fn new(ipc: DriverIpcClient, port: u16) -> Self {
        Self {
            ipc,
            updater: DesktopUpdater::new(),
            port,
        }
    }

    pub fn with_updater(ipc: DriverIpcClient, updater: DesktopUpdater, port: u16) -> Self {
        Self { ipc, updater, port }
    }

    pub async fn run(&self) {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                warn!("No se pudo iniciar servidor local de UI en {}: {}", addr, e);
                return;
            }
        };
        info!("Panel Desktop interactivo listo en http://{}", addr);

        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let ipc = self.ipc.clone();
            let updater = self.updater.clone();
            let port = self.port;
            tokio::spawn(async move {
                let _ = handle_http_conn(stream, ipc, updater, port).await;
            });
        }
    }
}

pub fn render_qr_svg(payload: &str) -> Result<String, String> {
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::H)
        .map_err(|e| e.to_string())?;
    let image = code
        .render::<svg::Color>()
        .min_dimensions(184, 184)
        .dark_color(svg::Color("#121212"))
        .light_color(svg::Color("#FFFFFF"))
        .quiet_zone(true)
        .build();
    Ok(image)
}

fn is_allowed_host_header(host: &str, port: u16) -> bool {
    let lower = host.trim().to_ascii_lowercase();
    lower == format!("127.0.0.1:{}", port)
        || lower == format!("localhost:{}", port)
        || lower == format!("[::1]:{}", port)
        || lower == "127.0.0.1"
        || lower == "localhost"
}

fn is_allowed_local_origin(origin: &str, port: u16) -> bool {
    let lower = origin.trim().trim_end_matches('/').to_ascii_lowercase();
    lower == format!("http://127.0.0.1:{}", port)
        || lower == format!("http://localhost:{}", port)
        || lower == format!("http://[::1]:{}", port)
}

fn is_valid_relay_url(raw: &str) -> bool {
    let t = raw.trim();
    if t.is_empty() || t.len() > 256 || t.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    if t.starts_with("https://") || t.starts_with("wss://") {
        let after = t.split("://").nth(1).unwrap_or("");
        return !after.is_empty() && !after.starts_with('/') && !after.contains('@');
    }
    t.starts_with("http://127.0.0.1")
        || t.starts_with("ws://127.0.0.1")
        || t.starts_with("http://localhost")
        || t.starts_with("ws://localhost")
}

async fn read_http_request(stream: &mut TcpStream) -> anyhow::Result<Option<(String, String)>> {
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    let mut header_end: Option<usize> = None;
    let mut content_length: usize = 0;
    let mut sent_continue = false;

    loop {
        if let Some(h_end) = header_end {
            if buf.len() >= h_end + content_length {
                let header_str = String::from_utf8_lossy(&buf[..h_end - 4]).to_string();
                let body_end = (h_end + content_length).min(buf.len());
                let body_str = String::from_utf8_lossy(&buf[h_end..body_end]).to_string();
                return Ok(Some((header_str, body_str)));
            }
        }

        let n = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.read(&mut tmp),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Timeout reading HTTP request"))??;

        if n == 0 {
            if let Some(h_end) = header_end {
                let header_str = String::from_utf8_lossy(&buf[..h_end - 4]).to_string();
                let body_str = String::from_utf8_lossy(&buf[h_end..]).to_string();
                return Ok(Some((header_str, body_str)));
            }
            return Ok(None);
        }

        buf.extend_from_slice(&tmp[..n]);
        if header_end.is_none() {
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let h_end = pos + 4;
                header_end = Some(h_end);
                let header_str = String::from_utf8_lossy(&buf[..pos]);
                let mut expect_100 = false;
                for line in header_str.lines().skip(1) {
                    if let Some((k, v)) = line.split_once(':') {
                        let key = k.trim();
                        let val = v.trim();
                        if key.eq_ignore_ascii_case("content-length") {
                            content_length = val.parse::<usize>().unwrap_or(0).min(1024 * 1024);
                        } else if key.eq_ignore_ascii_case("expect")
                            && val.to_ascii_lowercase().contains("100-continue")
                        {
                            expect_100 = true;
                        }
                    }
                }
                if expect_100 && buf.len() < h_end + content_length && !sent_continue {
                    sent_continue = true;
                    let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await;
                }
            } else if buf.len() > 64 * 1024 {
                anyhow::bail!("HTTP headers too large");
            }
        }
    }
}

async fn handle_http_conn(
    mut stream: TcpStream,
    ipc: DriverIpcClient,
    updater: DesktopUpdater,
    port: u16,
) -> anyhow::Result<()> {
    let Some((header_block, body_owned)) = read_http_request(&mut stream).await? else {
        return Ok(());
    };
    let body = body_owned.trim_end_matches('\0');
    let mut lines = header_block.lines();
    let first_line = lines.next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");

    let mut host_hdr: Option<&str> = None;
    let mut origin_hdr: Option<&str> = None;
    let mut sec_fetch_site: Option<&str> = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            if key.eq_ignore_ascii_case("host") {
                host_hdr = Some(val);
            } else if key.eq_ignore_ascii_case("origin") {
                origin_hdr = Some(val);
            } else if key.eq_ignore_ascii_case("sec-fetch-site") {
                sec_fetch_site = Some(val);
            }
        }
    }

    let host_ok = host_hdr.map(|h| is_allowed_host_header(h, port)).unwrap_or(false);
    let csrf_blocked = if method == "POST" {
        let bad_origin = origin_hdr
            .map(|o| !is_allowed_local_origin(o, port))
            .unwrap_or(false);
        let bad_fetch_site = sec_fetch_site
            .map(|s| !s.eq_ignore_ascii_case("same-origin") && !s.eq_ignore_ascii_case("none"))
            .unwrap_or(false);
        !host_ok || bad_origin || bad_fetch_site
    } else {
        !host_ok
    };

    if csrf_blocked {
        let body = r#"{"ok":false,"error":"Forbidden origin or host"}"#;
        let resp = format!(
            "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Frame-Options: DENY\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(resp.as_bytes()).await?;
        return Ok(());
    }

    let clean_path = path.split('?').next().unwrap_or("/");
    let (status, content_type, resp_body) = match (method, clean_path) {
        ("GET", "/") | ("GET", "/index.html") => (
            "200 OK",
            "text/html; charset=utf-8",
            EmbeddedUi::INDEX_HTML.to_string(),
        ),
        ("GET", "/i18n/en.json") => (
            "200 OK",
            "application/json; charset=utf-8",
            EmbeddedUi::EN_I18N_JSON.to_string(),
        ),
        ("GET", "/i18n/es.json") => (
            "200 OK",
            "application/json; charset=utf-8",
            EmbeddedUi::ES_I18N_JSON.to_string(),
        ),
        ("GET", "/api/state") => {
            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
            let state = ipc.get_live_state().await;
            updater
                .set_known_driver_version(state.snapshot.driver_version.clone())
                .await;
            let updates = updater.snapshot().await;
            let mut val = serde_json::to_value(&state).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(obj) = val.as_object_mut() {
                if let Ok(up_val) = serde_json::to_value(&updates) {
                    obj.insert("updates".to_string(), up_val);
                }
            }
            let json = val.to_string();
            ("200 OK", "application/json", json)
        }
        ("POST", "/api/qr") => match serde_json::from_str::<QrRequest>(body) {
            Ok(req) => match ipc.request_pairing_qr(&req.method).await {
                Ok(qr_data) => {
                    let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                    match render_qr_svg(&qr_data.uri) {
                    Ok(svg_markup) => {
                        let json = serde_json::json!({
                            "ok": true,
                            "method": qr_data.method,
                            "pair_token": qr_data.pair_token,
                            "expires_in_secs": qr_data.expires_in_secs,
                            "uri": qr_data.uri,
                            "share_link": qr_data.share_link,
                            "svg": svg_markup
                        })
                        .to_string();
                        ("200 OK", "application/json", json)
                    }
                    Err(err) => (
                        "500 Internal Server Error",
                        "application/json",
                        serde_json::json!({"ok": false, "error": err}).to_string(),
                    ),
                    }
                }
                Err(err) => (
                    "503 Service Unavailable",
                    "application/json",
                    serde_json::json!({"ok": false, "error": err.to_string()}).to_string(),
                ),
            },
            Err(_) => (
                "400 Bad Request",
                "application/json",
                r#"{"ok":false,"error":"JSON inválido"}"#.to_string(),
            ),
        },
        ("POST", "/api/command") => match serde_json::from_str::<CommandRequest>(body) {
            Ok(req) => {
                let res = match req.command.as_str() {
                    "pause" => ipc.send_command(IpcCommand::SetPaused { paused: true }).await,
                    "resume" => ipc.send_command(IpcCommand::SetPaused { paused: false }).await,
                    "disconnect" => ipc.send_command(IpcCommand::DisconnectDevice).await,
                    "set_tcp_enabled" => {
                        let r = ipc
                            .send_command(IpcCommand::SetTcpEnabled {
                                enabled: req.enabled.unwrap_or(true),
                            })
                            .await;
                        if r.is_ok() {
                            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                        r
                    }
                    "set_wfd_enabled" => {
                        let r = ipc
                            .send_command(IpcCommand::SetWifiDirectEnabled {
                                enabled: req.enabled.unwrap_or(true),
                            })
                            .await;
                        if r.is_ok() {
                            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                        r
                    }
                    "set_bt_enabled" => {
                        let r = ipc
                            .send_command(IpcCommand::SetBluetoothEnabled {
                                enabled: req.enabled.unwrap_or(true),
                            })
                            .await;
                        if r.is_ok() {
                            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                        r
                    }
                    "set_usb_enabled" => {
                        let r = ipc
                            .send_command(IpcCommand::SetUsbEnabled {
                                enabled: req.enabled.unwrap_or(true),
                            })
                            .await;
                        if r.is_ok() {
                            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                        r
                    }
                    "set_usb_priority" => {
                        let r = ipc
                            .send_command(IpcCommand::SetUsbPriority {
                                enabled: req.enabled.unwrap_or(true),
                            })
                            .await;
                        if r.is_ok() {
                            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                        r
                    }
                    "revoke" => match req.device_id.filter(|id| !id.trim().is_empty()) {
                        Some(device_id) => {
                            ipc.remove_saved_profile(&device_id).await;
                            let r = ipc
                                .send_command(IpcCommand::RevokeTrustedDevice { device_id })
                                .await;
                            if r.is_ok() {
                                let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                                let _ = ipc.send_command(IpcCommand::GetStatus).await;
                                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                            }
                            r
                        }
                        None => Err(anyhow::anyhow!("Falta device_id")),
                    },
                    "rename" => match (
                        req.device_id.filter(|id| !id.trim().is_empty()),
                        req.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
                    ) {
                        (Some(device_id), Some(name)) => {
                            ipc.rename_saved_profile(&device_id, &name).await;
                            let r = ipc
                                .send_command(IpcCommand::RenameTrustedDevice { device_id, name })
                                .await;
                            if r.is_ok() {
                                let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                                let _ = ipc.send_command(IpcCommand::GetStatus).await;
                                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                            }
                            r
                        }
                        _ => Err(anyhow::anyhow!("Falta device_id o name")),
                    },
                    "select_profile" => match req.device_id.filter(|id| !id.trim().is_empty()) {
                        Some(device_id) => {
                            ipc.select_saved_profile(&device_id).await;
                            let r = ipc
                                .send_command(IpcCommand::SelectTrustedDevice { device_id })
                                .await;
                            if r.is_ok() {
                                let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                                let _ = ipc.send_command(IpcCommand::GetStatus).await;
                                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                            }
                            r
                        }
                        None => Err(anyhow::anyhow!("Falta device_id")),
                    },
                    "save_ui_settings" => {
                        let cur = ipc.get_live_state().await.ui_settings;
                        let updated = crate::server::SyncUiSettings {
                            theme_mode: req.theme_mode.unwrap_or(cur.theme_mode),
                            use_windows_accent: req
                                .use_windows_accent
                                .unwrap_or(cur.use_windows_accent),
                            custom_accent_hex: req
                                .custom_accent_hex
                                .unwrap_or(cur.custom_accent_hex),
                            auto_update_app: req.auto_update_app.unwrap_or(cur.auto_update_app),
                            auto_update_driver: req
                                .auto_update_driver
                                .unwrap_or(cur.auto_update_driver),
                            language: req.language.unwrap_or(cur.language),
                        };
                        ipc.update_ui_settings(updated).await;
                        Ok(())
                    }
                    "check_updates" => {
                        let live = ipc.get_live_state().await;
                        let updater_bg = updater.clone();
                        let drv_ver = live.snapshot.driver_version.clone();
                        let auto_app = live.ui_settings.auto_update_app;
                        let auto_drv = live.ui_settings.auto_update_driver;
                        tokio::spawn(async move {
                            updater_bg.check_all(drv_ver, auto_app, auto_drv).await;
                        });
                        Ok(())
                    }
                    "install_app_update" => {
                        let updater_bg = updater.clone();
                        tokio::spawn(async move {
                            let _ = updater_bg.install_app_update().await;
                        });
                        Ok(())
                    }
                    "install_driver_update" => {
                        let updater_bg = updater.clone();
                        tokio::spawn(async move {
                            let _ = updater_bg.install_driver_update().await;
                        });
                        Ok(())
                    }
                    "restart_app" => updater.restart_running_app().await,
                    "set_tcp_network" => {
                        let r = ipc
                            .send_command(IpcCommand::SetTcpNetworkConfig {
                                tcp_ip: req.tcp_ip,
                                tcp_port: req.tcp_port,
                                reset: req.reset.unwrap_or(false),
                            })
                            .await;
                        if r.is_ok() {
                            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                            tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                        }
                        r
                    }
                    "set_auto_accept_mobile_switch" => {
                        let r = ipc
                            .send_command(IpcCommand::SetAutoAcceptMobileSwitch {
                                enabled: req.enabled.unwrap_or(false),
                            })
                            .await;
                        if r.is_ok() {
                            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                        }
                        r
                    }
                    "respond_mobile_switch" => {
                        let req_id = req.request_id.unwrap_or_default();
                        ipc.clear_pending_mobile_switch(Some(&req_id)).await;
                        let r = ipc
                            .send_command(IpcCommand::RespondMobileSwitch {
                                request_id: req_id,
                                accept: req.accept.unwrap_or(false),
                            })
                            .await;
                        if r.is_ok() {
                            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                            let _ = ipc.send_command(IpcCommand::GetStatus).await;
                            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                        }
                        r
                    }
                    "dismiss_invalid_pairing" => {
                        ipc.clear_invalid_pairing_notice().await;
                        if req.delete_profile.unwrap_or(false) {
                            if let Some(device_id) = req.device_id.filter(|id| !id.trim().is_empty()) {
                                ipc.remove_saved_profile(&device_id).await;
                                let r = ipc
                                    .send_command(IpcCommand::RevokeTrustedDevice { device_id })
                                    .await;
                                if r.is_ok() {
                                    let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
                                }
                                r
                            } else {
                                Ok(())
                            }
                        } else {
                            Ok(())
                        }
                    }
                    _ => Err(anyhow::anyhow!("Comando desconocido")),
                };
                match res {
                    Ok(()) => ("200 OK", "application/json", r#"{"ok":true}"#.to_string()),
                    Err(e) => (
                        "503 Service Unavailable",
                        "application/json",
                        serde_json::json!({"ok": false, "error": e.to_string()}).to_string(),
                    ),
                }
            }
            Err(_) => (
                "400 Bad Request",
                "application/json",
                r#"{"ok":false}"#.to_string(),
            ),
        },
        ("POST", "/api/relay") => match serde_json::from_str::<RelayRequest>(body) {
            Ok(req) => {
                let valid = req
                    .url
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(is_valid_relay_url)
                    .unwrap_or(true);
                if !valid {
                    (
                        "400 Bad Request",
                        "application/json",
                        r#"{"ok":false,"error":"La URL del Relay debe usar https:// o wss://"}"#.to_string(),
                    )
                } else {
                    match ipc.send_command(IpcCommand::SetRelayUrl { url: req.url }).await {
                        Ok(()) => ("200 OK", "application/json", r#"{"ok":true}"#.to_string()),
                        Err(e) => (
                            "503 Service Unavailable",
                            "application/json",
                            serde_json::json!({"ok": false, "error": e.to_string()}).to_string(),
                        ),
                    }
                }
            }
            Err(_) => (
                "400 Bad Request",
                "application/json",
                r#"{"ok":false}"#.to_string(),
            ),
        },
        _ => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Not Found".to_string(),
        ),
    };

    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nX-Frame-Options: DENY\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
        status,
        content_type,
        resp_body.as_bytes().len(),
        resp_body
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_qr_svg_outputs_valid_svg() {
        let svg = render_qr_svg("nfcbridge://192.168.15.10:45362?transport=tcp&pair=PAIR-1234")
            .expect("svg render should succeed");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn test_host_origin_and_relay_url_validation() {
        assert!(is_allowed_host_header("127.0.0.1:45363", 45363));
        assert!(is_allowed_host_header("localhost:45363", 45363));
        assert!(!is_allowed_host_header("evil.com", 45363));
        assert!(is_allowed_local_origin("http://127.0.0.1:45363", 45363));
        assert!(!is_allowed_local_origin("https://evil.com", 45363));
        assert!(is_valid_relay_url("https://nfc-bridge-relay.example.workers.dev"));
        assert!(!is_valid_relay_url("ws://public.example.com"));
        assert!(is_valid_relay_url("ws://127.0.0.1:8787"));
    }

    #[test]
    fn test_embedded_i18n_catalogs_are_valid_and_in_sync() {
        let en: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(EmbeddedUi::EN_I18N_JSON).expect("en.json should parse");
        let es: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(EmbeddedUi::ES_I18N_JSON).expect("es.json should parse");

        for required in [
            "title_options_sync",
            "title_options_tools",
            "settings_appearance_header",
            "settings_language_system",
            "settings_language_en",
            "settings_language_es",
            "settings_mode_portable",
            "nav_appearance",
        ] {
            assert!(en.contains_key(required), "en.json missing {required}");
            assert!(es.contains_key(required), "es.json missing {required}");
        }

        let en_keys: std::collections::BTreeSet<_> = en.keys().cloned().collect();
        let es_keys: std::collections::BTreeSet<_> = es.keys().cloned().collect();
        assert_eq!(en_keys, es_keys, "en.json and es.json key sets must match");
    }

    #[tokio::test]
    async fn test_http_routes_serve_settings_query_and_i18n() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let ipc = DriverIpcClient::new();
        let updater = DesktopUpdater::new();

        let server_task = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let ipc = ipc.clone();
                let updater = updater.clone();
                handle_http_conn(stream, ipc, updater, port).await.unwrap();
            }
        });

        // 1. GET /?view=settings -> 200 OK text/html
        let mut s1 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let req1 = format!("GET /?view=settings HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        s1.write_all(req1.as_bytes()).await.unwrap();
        let mut resp1 = String::new();
        s1.read_to_string(&mut resp1).await.unwrap();
        assert!(resp1.starts_with("HTTP/1.1 200 OK\r\n"), "resp1={}", &resp1[..resp1.len().min(120)]);
        assert!(resp1.contains("Content-Type: text/html; charset=utf-8"));
        assert!(resp1.contains("settings-sidebar"));

        // 2. GET /i18n/en.json -> 200 OK application/json
        let mut s2 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let req2 = format!("GET /i18n/en.json HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        s2.write_all(req2.as_bytes()).await.unwrap();
        let mut resp2 = String::new();
        s2.read_to_string(&mut resp2).await.unwrap();
        assert!(resp2.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp2.contains("\"settings_language_system\""));

        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_split_packet_post_save_ui_settings() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let ipc = DriverIpcClient::new();
        let updater = DesktopUpdater::new();
        let prev_settings = ipc.get_live_state().await.ui_settings;

        let ipc_srv = ipc.clone();
        let server_task = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let ipc = ipc_srv.clone();
                let updater = updater.clone();
                handle_http_conn(stream, ipc, updater, port).await.unwrap();
            }
        });

        // Simulate WebView2/Chromium splitting headers and JSON body into two TCP writes
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let body = r##"{"command":"save_ui_settings","theme_mode":"light","language":"en","use_windows_accent":false,"custom_accent_hex":"#10B981"}"##;
        let headers = format!(
            "POST /api/command HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nSec-Fetch-Site: same-origin\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        s.write_all(headers.as_bytes()).await.unwrap();
        s.flush().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        s.write_all(body.as_bytes()).await.unwrap();
        s.flush().await.unwrap();

        let mut resp = String::new();
        s.read_to_string(&mut resp).await.unwrap();
        assert!(resp.starts_with("HTTP/1.1 200 OK\r\n"), "resp={resp}");
        assert!(resp.contains(r#"{"ok":true}"#));

        // Verify GET /api/state reflects updated settings
        let mut s2 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let req2 = format!("GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        s2.write_all(req2.as_bytes()).await.unwrap();
        let mut resp2 = String::new();
        s2.read_to_string(&mut resp2).await.unwrap();
        assert!(resp2.contains(r#""theme_mode":"light""#));
        assert!(resp2.contains(r#""language":"en""#));
        assert!(resp2.contains(r##""custom_accent_hex":"#10B981""##));

        server_task.await.unwrap();
        ipc.update_ui_settings(prev_settings).await;
    }
}
