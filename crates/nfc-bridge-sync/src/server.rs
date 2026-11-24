//! # Driver IPC Client (Named Pipe)
//!
//! `nfc-bridge-sync` NO abre puertos de red ni habla directamente con el celular.
//! Toda la conectividad (TCP, UDP Auto-Discovery, USB AOA, Bluetooth, Wi-Fi Direct)
//! y la emulación PC/SC residen exclusivamente en `nfc-bridge-driver`.
//!
//! Este módulo conecta la interfaz de escritorio con el driver a través del canal local
//! `\\.\pipe\nfc_bridge_driver`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn};

use crate::driver_manager::DRIVER_NAMED_PIPE;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "command", content = "params")]
pub enum IpcCommand {
    GetStatus,
    SetPaused { paused: bool },
    SetUsbPriority { enabled: bool },
    DisconnectDevice,
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

pub struct DriverIpcClient;

impl DriverIpcClient {
    pub fn new() -> Self {
        Self
    }

    /// Mantiene conexión IPC persistente con `nfc-bridge-driver`.
    /// Si el driver no está iniciado, espera sin abrir ningún puerto de red alternativo.
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
                            "Esperando a NFC Bridge Driver ({})... Sin el driver activo no es posible conectar por TCP, USB AOA, Bluetooth ni Wi-Fi Direct.",
                            DRIVER_NAMED_PIPE
                        );
                    }
                }
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

            let (reader, _writer) = tokio::io::split(client);
            let mut lines = BufReader::new(reader).lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(event) = serde_json::from_str::<IpcEvent>(&line) {
                    Self::handle_driver_event(event);
                }
            }

            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Named Pipe IPC solo soportado en Windows");
        }
    }

    fn handle_driver_event(event: IpcEvent) {
        match event {
            IpcEvent::Status {
                connected,
                device_name,
                latency_ms,
                paused,
                tag_present,
                tag_uid,
            } => {
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
            IpcEvent::DeviceConnected {
                device_name,
                endpoint,
            } => {
                info!("[Driver] Dispositivo Android conectado: '{}' vía {}", device_name, endpoint);
            }
            IpcEvent::DeviceDisconnected => {
                info!("[Driver] Dispositivo Android desconectado.");
            }
            IpcEvent::TagAttached {
                uid,
                technologies,
                is_ntag215,
            } => {
                info!(
                    "[Driver] Etiqueta NFC detectada: UID={} | Techs={:?} | NTAG215={}",
                    uid, technologies, is_ntag215
                );
            }
            IpcEvent::TagDetached => {
                info!("[Driver] Etiqueta NFC retirada del campo.");
            }
            IpcEvent::LatencyUpdated { .. } => {}
            IpcEvent::Error { message } => {
                warn!("[Driver Error] {}", message);
            }
        }
    }
}
