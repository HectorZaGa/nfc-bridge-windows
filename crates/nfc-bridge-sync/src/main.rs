use anyhow::Result;
use nfc_bridge_sync::{AppConfig, BridgeSyncServer, DriverManager, DriverStatus, EmbeddedUi};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = AppConfig::default();

    println!("==================================================");
    println!("          NFC Bridge Sync - Windows Lite          ");
    if config.is_portable {
        println!("             [ MODO PORTABLE ACTIVO ]             ");
    }
    println!("==================================================");

    info!("Directorio de datos: {:?}", config.data_dir);
    info!("Interfaz Stitch integrada: {} KB incrustados en .exe", 
        (EmbeddedUi::INDEX_HTML.len() + EmbeddedUi::TRAY_HTML.len() + EmbeddedUi::CASES_HTML.len()) / 1024
    );

    let driver_mgr = DriverManager::new();
    let status = driver_mgr.check_driver_status();

    match status {
        DriverStatus::Running => {
            info!("Driver Virtual PC/SC detectado y activo.");
        }
        DriverStatus::InstalledStopped => {
            info!("Driver instalado pero el canal Named Pipe está inactivo.");
        }
        DriverStatus::NotInstalled => {
            info!("Driver no detectado. Puede descargarse la última versión desde GitHub.");
        }
    }

    info!("Identidad de esta PC: {}", config.pc_name);

    let server = BridgeSyncServer::new(config.pc_name, config.tcp_port);
    server.run().await?;

    Ok(())
}
