use anyhow::Result;
use nfc_bridge_sync::{AppConfig, BridgeSyncServer, DriverManager, DriverStatus, EmbeddedUi};
use std::net::UdpSocket;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = AppConfig::load_or_default();
    let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());

    println!("==================================================");
    println!("          NFC Bridge Sync - Windows Lite          ");
    if config.is_portable {
        println!("             [ MODO PORTABLE ACTIVO ]             ");
    }
    println!("==================================================");
    println!(" [IP de esta PC] : {}", local_ip);
    println!(" [Puerto TCP]    : {}", config.tcp_port);
    println!(" [Endpoint móvil]: {}:{}", local_ip, config.tcp_port);
    println!(" [Transporte USB]: AOA Nativo vía NFC Bridge Driver");
    println!("==================================================");

    info!("Directorio de datos: {:?}", config.data_dir);
    info!(
        "Interfaz Stitch integrada: {} KB incrustados en .exe",
        (EmbeddedUi::INDEX_HTML.len() + EmbeddedUi::TRAY_HTML.len() + EmbeddedUi::CASES_HTML.len()) / 1024
    );

    let driver_mgr = DriverManager::new();
    let mut status = driver_mgr.check_driver_status();

    if status == DriverStatus::NotInstalled {
        if let Some(local_driver) = driver_mgr.find_local_driver_binary() {
            info!("Driver no detectado en el sistema, pero se encontró binario local en {:?}.", local_driver);
            info!("Iniciando driver virtual silenciosamente en segundo plano...");
            if let Ok(_child) = driver_mgr.start_local_driver_background(&local_driver) {
                std::thread::sleep(std::time::Duration::from_millis(500));
                status = driver_mgr.check_driver_status();
            }
        }
    }

    match status {
        DriverStatus::Running => {
            info!("Driver Virtual PC/SC + USB AOA detectado y activo (Named Pipe listo).");
        }
        DriverStatus::InstalledStopped => {
            info!("Driver instalado pero el canal Named Pipe está inactivo.");
        }
        DriverStatus::NotInstalled => {
            info!("Driver no detectado. El instalador o paquete completo incluye el driver PC/SC y USB AOA.");
        }
    }

    info!("Identidad de esta PC: {}", config.pc_name);

    let server = BridgeSyncServer::new(config.pc_name, config.tcp_port);
    server.run().await?;

    Ok(())
}
