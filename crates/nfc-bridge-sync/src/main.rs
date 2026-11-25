use anyhow::Result;
use nfc_bridge_sync::ui_http::DEFAULT_UI_HTTP_PORT;
use nfc_bridge_sync::{
    AppConfig, DesktopUiServer, DriverIpcClient, DriverManager, DriverStatus, EmbeddedUi,
};
use std::net::UdpSocket;
use tracing::{info, warn, Level};
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
    println!(" [Identidad PC]  : {}", config.pc_name);
    println!(" [Panel Web/UI]  : http://127.0.0.1:{}", DEFAULT_UI_HTTP_PORT);
    println!(" [Arquitectura]  : Cliente Visual IPC -> Driver");
    println!("==================================================");

    info!("Directorio de datos: {:?}", config.data_dir);
    info!(
        "Interfaz Stitch integrada: {} KB incrustados en .exe",
        (EmbeddedUi::INDEX_HTML.len() + EmbeddedUi::TRAY_HTML.len() + EmbeddedUi::CASES_HTML.len()) / 1024
    );

    let driver_mgr = DriverManager::new();
    let mut status = driver_mgr.check_driver_status();

    if status == DriverStatus::InstalledStopped {
        info!("Servicio NFC Bridge Driver instalado pero detenido. Intentando iniciar...");
        let _ = std::process::Command::new("sc.exe")
            .args(["start", "NfcBridgeDriver"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(600));
        status = driver_mgr.check_driver_status();
    }

    if status == DriverStatus::NotInstalled {
        if let Some(local_driver) = driver_mgr.find_local_driver_binary() {
            info!("Driver no activo, iniciando binario local silencioso {:?}...", local_driver);
            if let Ok(_child) = driver_mgr.start_local_driver_background(&local_driver) {
                std::thread::sleep(std::time::Duration::from_millis(600));
                status = driver_mgr.check_driver_status();
            }
        }
    }

    match status {
        DriverStatus::Running => {
            info!("NFC Bridge Driver detectado y activo (TCP 45362, UDP Discovery, USB AOA y PC/SC bajo control del driver).");
        }
        DriverStatus::InstalledStopped => {
            warn!("NFC Bridge Driver está instalado pero detenido. No habrá conexión hasta iniciarlo.");
        }
        DriverStatus::NotInstalled => {
            warn!("DRIVER NO DETECTADO: Instala o inicia `nfc-bridge-driver` para habilitar conexiones (TCP, USB AOA, Bluetooth, Wi-Fi Direct).");
        }
    }

    // Conectarse exclusivamente al driver vía Named Pipe IPC (nunca abre puertos de red por su cuenta)
    let ipc_client = DriverIpcClient::new();
    let ui_server = DesktopUiServer::new(ipc_client.clone(), DEFAULT_UI_HTTP_PORT);
    tokio::spawn(async move {
        ui_server.run().await;
    });

    if std::env::args().any(|a| a == "--open-ui" || a == "--ui") {
        let url = format!("http://127.0.0.1:{}", DEFAULT_UI_HTTP_PORT);
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn();
    }

    ipc_client.run_forever().await?;

    Ok(())
}
