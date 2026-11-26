//! Arranque unificado para los binarios de escritorio (`nfc-bridge-sync` y `nfc-bridge-tools`).

use crate::config::AppConfig;
use crate::driver_manager::{DriverManager, DriverStatus};
use crate::ipc::DriverIpcClient;
use crate::platform::{hidden_command, replace_previous_sync_processes};
use crate::updater::{cleanup_stale_update_files, DesktopUpdater};
use anyhow::Result;
use std::net::UdpSocket;
use tracing::{info, warn};

pub struct DesktopLaunchSpec {
    pub product_slug: &'static str,
    pub display_name: &'static str,
    pub version: &'static str,
    pub build_id: &'static str,
    pub single_instance_sync: bool,
    pub sync_product_meta_to_ipc: bool,
    pub app_ico_bytes: &'static [u8],
}

pub fn local_outbound_ip() -> String {
    let detected = UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|s| s.connect("8.8.8.8:80").ok().map(|_| s))
        .and_then(|s| s.local_addr().ok())
        .map(|addr| addr.ip().to_string());
    detected.unwrap_or_else(|| "127.0.0.1".to_string())
}

fn ensure_driver_running() {
    let driver_mgr = DriverManager::new();
    let mut status = driver_mgr.check_driver_status();

    if status == DriverStatus::InstalledStopped {
        info!("Servicio NFC Bridge Driver instalado pero detenido. Intentando iniciar...");
        let _ = hidden_command("sc.exe")
            .args(["start", "NfcBridgeDriver"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(600));
        status = driver_mgr.check_driver_status();
    }

    match status {
        DriverStatus::Running => {
            info!("NFC Bridge Driver detectado y activo (servicio de Windows).");
        }
        DriverStatus::InstalledStopped => {
            warn!("NFC Bridge Driver está instalado pero detenido. No habrá conexión hasta iniciarlo.");
        }
        DriverStatus::NotInstalled => {
            warn!("DRIVER NO INSTALADO: Se intentará descargar e instalar el servicio NfcBridgeDriver desde GitHub Releases.");
        }
    }
}

pub fn run_desktop_app(spec: DesktopLaunchSpec, config: AppConfig) -> Result<()> {
    cleanup_stale_update_files();
    if spec.single_instance_sync && replace_previous_sync_processes() > 0 {
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    let local_ip = local_outbound_ip();
    info!(
        "{} v{} ({}) | IP={} | PC={} | Portable={} | Modo=desktop-native",
        spec.display_name,
        spec.version,
        spec.build_id,
        local_ip,
        config.pc_name,
        config.is_portable
    );
    info!("Directorio de datos: {:?}", config.data_dir);

    ensure_driver_running();

    let rt = tokio::runtime::Runtime::new()?;
    let ipc_client = DriverIpcClient::new();
    let updater = DesktopUpdater::new();

    rt.block_on(async {
        if spec.sync_product_meta_to_ipc {
            ipc_client
                .set_product_meta(spec.version, spec.build_id)
                .await;
        }
        updater
            .set_product_identity(spec.product_slug, spec.display_name, spec.version)
            .await;
    });

    let rt_handle = rt.handle().clone();
    let ipc_for_ui = ipc_client.clone();
    let upd_for_ui = updater.clone();

    let _runtime_handle = std::thread::spawn(move || {
        rt.block_on(async move {
            {
                let updater_init = updater.clone();
                let ipc_init = ipc_client.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1600)).await;
                    let live = ipc_init.get_live_state().await;
                    updater_init
                        .check_all(
                            live.snapshot.driver_version,
                            live.ui_settings.auto_update_app,
                            live.ui_settings.auto_update_driver,
                        )
                        .await;
                });
            }

            let _ = ipc_client.run_forever().await;
        });
    });

    let force_open_ui = std::env::args().any(|a| a == "--open-ui");
    let start_minimized = std::env::args().any(|a| a == "--minimized" || a == "--background");
    let initial_visible = force_open_ui || !start_minimized;

    if spec.product_slug == "sync" {
        if config.autostart_with_windows
            && !crate::platform::is_windows_autostart_enabled("NfcBridgeSync")
        {
            let _ = crate::platform::set_windows_autostart("NfcBridgeSync", true);
        }
    }

    #[cfg(target_os = "windows")]
    crate::ui::run_desktop_ui(
        spec.product_slug,
        spec.display_name,
        spec.app_ico_bytes,
        ipc_for_ui,
        upd_for_ui,
        rt_handle,
        config,
        initial_visible,
        force_open_ui,
    )?;

    Ok(())
}
