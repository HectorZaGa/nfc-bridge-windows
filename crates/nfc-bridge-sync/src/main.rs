use anyhow::Result;
use nfc_bridge_sync::{BridgeSyncServer, DriverManager, DriverStatus};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    println!("==================================================");
    println!("          NFC Bridge Sync - Windows Lite          ");
    println!("==================================================");

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

    let pc_name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Master PC Race".to_string());
    info!("Identidad de esta PC: {}", pc_name);

    let server = BridgeSyncServer::new(pc_name, 8765);
    server.run().await?;

    Ok(())
}
