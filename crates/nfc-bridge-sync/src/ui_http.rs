//! Servidor HTTP local ligero (127.0.0.1:45363) para servir la UI de escritorio Stitch
//! (`EmbeddedUi::INDEX_HTML`), generar códigos QR SVG offline de alta precisión y puentear
//! comandos en tiempo real hacia `nfc-bridge-driver` vía IPC Named Pipe.

use qrcode::render::svg;
use qrcode::QrCode;
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
            tokio::spawn(async move {
                let _ = handle_http_conn(stream, ipc).await;
            });
        }
    }
}

pub fn render_qr_svg(payload: &str) -> Result<String, String> {
    let code = QrCode::new(payload.as_bytes()).map_err(|e| e.to_string())?;
    let image = code
        .render::<svg::Color>()
        .min_dimensions(184, 184)
        .dark_color(svg::Color("#121212"))
        .light_color(svg::Color("#FFFFFF"))
        .quiet_zone(true)
        .build();
    Ok(image)
}

async fn handle_http_conn(mut stream: TcpStream, ipc: DriverIpcClient) -> anyhow::Result<()> {
    let mut buf = vec![0u8; 16 * 1024];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }

    let req_str = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req_str.lines();
    let first_line = lines.next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");

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
                Ok(qr_data) => match render_qr_svg(&qr_data.uri) {
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
                },
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
            Ok(req) => match ipc.send_command(IpcCommand::SetRelayUrl { url: req.url }).await {
                Ok(()) => ("200 OK", "application/json", r#"{"ok":true}"#.to_string()),
                Err(e) => (
                    "503 Service Unavailable",
                    "application/json",
                    serde_json::json!({"ok": false, "error": e.to_string()}).to_string(),
                ),
            },
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
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
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
}
