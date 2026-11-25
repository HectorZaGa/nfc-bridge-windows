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
}

#[derive(Deserialize)]
struct RelayRequest {
    url: Option<String>,
}

pub struct DesktopUiServer {
    ipc: DriverIpcClient,
    port: u16,
}

impl DesktopUiServer {
    pub fn new(ipc: DriverIpcClient, port: u16) -> Self {
        Self { ipc, port }
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
            let port = self.port;
            tokio::spawn(async move {
                let _ = handle_http_conn(stream, ipc, port).await;
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

async fn handle_http_conn(mut stream: TcpStream, ipc: DriverIpcClient, port: u16) -> anyhow::Result<()> {
    let mut buf = vec![0u8; 16 * 1024];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }

    let req_str = String::from_utf8_lossy(&buf[..n]);
    let header_block = req_str.split("\r\n\r\n").next().unwrap_or("");
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

    let body = req_str
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .trim_end_matches('\0');

    let (status, content_type, resp_body) = match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => (
            "200 OK",
            "text/html; charset=utf-8",
            EmbeddedUi::INDEX_HTML.to_string(),
        ),
        ("GET", "/api/state") => {
            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
            let state = ipc.get_live_state().await;
            let json = serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string());
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
                        };
                        ipc.update_ui_settings(updated).await;
                        Ok(())
                    }
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
}
