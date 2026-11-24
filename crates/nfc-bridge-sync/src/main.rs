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

fn run_adb_hidden(candidate: &std::path::Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = std::process::Command::new(candidate);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output()
}

fn configure_adb_reverse(
    port: u16,
    active_devices: &mut std::collections::HashSet<String>,
    unauthorized_devices: &mut std::collections::HashSet<String>,
    warned_missing_adb: &mut bool,
) {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();

    let adb_candidates = [
        exe_dir.join("adb.exe"),
        exe_dir.join("platform-tools").join("adb.exe"),
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
    let mut found_adb = false;
    for candidate in &adb_candidates {
        if candidate.as_os_str().is_empty() {
            continue;
        }
        if candidate == &std::path::PathBuf::from("adb") || candidate.exists() {
            // 1. Obtener la lista de dispositivos conectados y autorizados
            let Ok(dev_out) = run_adb_hidden(candidate, &["devices"]) else { continue; };
            if !dev_out.status.success() { continue; };
            found_adb = true;

            let stdout_str = String::from_utf8_lossy(&dev_out.stdout);
            let mut current_connected = std::collections::HashSet::new();
            let mut current_unauthorized = std::collections::HashSet::new();

            for line in stdout_str.lines().skip(1) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let mut parts = trimmed.split_whitespace();
                let Some(serial) = parts.next() else { continue; };
                let Some(state) = parts.next() else { continue; };
                match state {
                    "device" => {
                        current_connected.insert(serial.to_string());
                    }
                    "unauthorized" => {
                        current_unauthorized.insert(serial.to_string());
                        if !unauthorized_devices.contains(serial) {
                            tracing::warn!(
                                "Dispositivo USB [{}] detectado pero sin autorizar: acepta 'Depuración por USB' en la pantalla del teléfono.",
                                serial
                            );
                        }
                    }
                    _ => {}
                }
            }
            *unauthorized_devices = current_unauthorized;

            // Notificar dispositivos desconectados
            active_devices.retain(|d| {
                let still_connected = current_connected.contains(d);
                if !still_connected {
                    tracing::info!("Dispositivo USB [{}] desconectado de ADB Reverse", d);
                }
                still_connected
            });

            // Configurar reverse para cada dispositivo nuevo conectado
            for serial in &current_connected {
                if !active_devices.contains(serial) {
                    if let Ok(out) = run_adb_hidden(candidate, &["-s", serial, "reverse", &reverse_arg, &reverse_arg]) {
                        if out.status.success() {
                            tracing::info!(
                                "ADB Reverse activado con éxito en [{}] : {} -> {} (cable USB listo para Android)",
                                serial, reverse_arg, reverse_arg
                            );
                            active_devices.insert(serial.clone());
                        } else {
                            let err = String::from_utf8_lossy(&out.stderr);
                            tracing::warn!("Error al activar ADB Reverse en [{}]: {}", serial, err.trim());
                        }
                    }
                }
            }
            return;
        }
    }

    if !found_adb && !*warned_missing_adb {
        *warned_missing_adb = true;
        tracing::warn!("No se encontró adb.exe en PATH ni en Android SDK; el túnel USB ADB Reverse no estará disponible hasta instalar platform-tools.");
    }
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
                // Dar tiempo breve para que el pipe se inicialice
                std::thread::sleep(std::time::Duration::from_millis(500));
                status = driver_mgr.check_driver_status();
            }
        }
    }

    match status {
        DriverStatus::Running => {
            info!("Driver Virtual PC/SC detectado y activo (Named Pipe listo).");
        }
        DriverStatus::InstalledStopped => {
            info!("Driver instalado pero el canal Named Pipe está inactivo.");
        }
        DriverStatus::NotInstalled => {
            info!("Driver no detectado. El instalador o versión completa incluye el driver PC/SC.");
        }
    }

    info!("Identidad de esta PC: {}", config.pc_name);

    let port = config.tcp_port;
    tokio::spawn(async move {
        let mut active_devices = std::collections::HashSet::new();
        let mut unauthorized_devices = std::collections::HashSet::new();
        let mut warned_missing_adb = false;
        loop {
            configure_adb_reverse(port, &mut active_devices, &mut unauthorized_devices, &mut warned_missing_adb);
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
    });

    let server = BridgeSyncServer::new(config.pc_name, config.tcp_port);
    server.run().await?;

    Ok(())
}
