//! # Bridge Sync Server
//!
//! Servidor TCP asíncrono que escucha conexiones de la aplicación Android NFC Bridge
//! en el puerto 8765 y gestiona el handshake y la transferencia de tramas BAP.

use anyhow::{Context, Result};
use bytes::BytesMut;
use core_proto::{FrameCodec, MessageType, NfcBridgeFrame};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

pub const DEFAULT_TCP_PORT: u16 = 45362;

pub struct BridgeSyncServer {
    port: u16,
    pc_name: String,
}

impl BridgeSyncServer {
    pub fn new(pc_name: String, port: u16) -> Self {
        Self { pc_name, port }
    }

    pub async fn run(&self) -> Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("No se pudo enlazar el servidor TCP en {}", addr))?;

        info!("NFC Bridge Sync escuchando conexiones en {}", addr);

        loop {
            match listener.accept().await {
                Ok((socket, peer_addr)) => {
                    info!("Nuevo dispositivo conectado desde {}", peer_addr);
                    let pc_name = self.pc_name.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(socket, peer_addr, pc_name).await {
                            warn!("Conexión con {} terminada: {}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error al aceptar conexión TCP: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        mut socket: TcpStream,
        peer_addr: SocketAddr,
        pc_name: String,
    ) -> Result<()> {
        let mut buffer = BytesMut::with_capacity(4096);
        let mut seq_counter: u16 = 0;

        loop {
            let n = socket.read_buf(&mut buffer).await?;
            if n == 0 {
                info!("El cliente {} cerró la conexión.", peer_addr);
                break;
            }

            // Procesar todas las tramas completas presentes en el buffer
            while let Some(frame) = FrameCodec::decode(&mut buffer)? {
                seq_counter = seq_counter.wrapping_add(1);
                match frame.message_type {
                    MessageType::HelloSyn => {
                        info!("Recibido HELLO_SYN de {}", peer_addr);
                        let response_payload = format!(r#"{{"pc_name":"{}","status":"ready"}}"#, pc_name);
                        let ack = NfcBridgeFrame::new(
                            MessageType::HelloAck,
                            seq_counter,
                            bytes::Bytes::copy_from_slice(response_payload.as_bytes()),
                        );
                        let encoded = FrameCodec::encode(&ack);
                        socket.write_all(&encoded).await?;
                    }
                    MessageType::Heartbeat => {
                        let ack = NfcBridgeFrame::heartbeat_ack(seq_counter);
                        let encoded = FrameCodec::encode(&ack);
                        socket.write_all(&encoded).await?;
                    }
                    MessageType::TagAttached => {
                        info!("Etiqueta NFC detectada por el cliente Android!");
                    }
                    MessageType::TagDetached => {
                        info!("Etiqueta NFC retirada.");
                    }
                    MessageType::TransceiveResp => {
                        info!("Respuesta APDU recibida ({} bytes)", frame.payload.len());
                    }
                    MessageType::Disconnect => {
                        info!("Solicitud de desconexión recibida de {}", peer_addr);
                        break;
                    }
                    _ => {
                        // Otros tipos de trama
                    }
                }
            }
        }

        Ok(())
    }
}
