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

fn configure_adb_reverse(port: u16) {
    let adb_candidates = [
        std::path::PathBuf::from("adb"),
        std::env::var("LOCALAPPDATA")
            .map(|l| std::path::PathBuf::from(l).join(r"Android\Sdk\platform-tools\adb.exe"))
            .unwrap_or_default(),
        std::env::var("ANDROID_HOME")
            .map(|a| std::path::PathBuf::from(a).join(r"platform-tools\adb.exe"))
            .unwrap_or_default(),
        std::env::var("ANDROID_SDK_ROOT")
            .map(|a| std::path::PathBuf::from(a).join(r"platform-tools\adb.exe"))
            .unwrap_or_default(),
    ];

    let reverse_arg = format!("tcp:{}", port);
    for candidate in &adb_candidates {
        if candidate.as_os_str().is_empty() {
            continue;
        }
        if candidate == &std::path::PathBuf::from("adb") || candidate.exists() {
            let output = std::process::Command::new(candidate)
                .args(["reverse", &reverse_arg, &reverse_arg])
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    tracing::info!(
                        "ADB Reverse activado con éxito: {} -> {} (cable USB listo para Android)",
                        reverse_arg, reverse_arg
                    );
                    return;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = AppConfig::default();
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
    println!("==================================================");

    info!("Directorio de datos: {:?}", config.data_dir);
    info!(
        "Interfaz Stitch integrada: {} KB incrustados en .exe",
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

    let port = config.tcp_port;
    tokio::spawn(async move {
        loop {
            configure_adb_reverse(port);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    let server = BridgeSyncServer::new(config.pc_name, config.tcp_port);
    server.run().await?;

    Ok(())
}
