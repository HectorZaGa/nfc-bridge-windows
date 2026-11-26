#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use nfc_bridge_sync::ui_http::DEFAULT_UI_HTTP_PORT;
use nfc_bridge_sync::updater::cleanup_stale_update_files;
use nfc_bridge_sync::{
    AppConfig, DesktopUiServer, DesktopUpdater, DriverIpcClient, DriverManager, DriverStatus,
    EmbeddedUi,
};
use std::net::UdpSocket;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}

fn replace_previous_sync_processes() -> usize {
    #[cfg(windows)]
    {
        use std::mem::{size_of, zeroed};
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
            PROCESS_TERMINATE,
        };

        let self_pid = std::process::id();
        let mut killed = 0usize;

        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return 0;
            }

            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snap, &mut entry) != 0 {
                loop {
                    let pid = entry.th32ProcessID;
                    if pid != 0 && pid != self_pid {
                        let end = entry
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExeFile.len());
                        let exe_name = String::from_utf16_lossy(&entry.szExeFile[..end])
                            .to_lowercase();

                        if exe_name.starts_with("nfc-bridge-sync")
                            || exe_name.starts_with("nfc bridge sync")
                        {
                            let proc_handle =
                                OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, pid);
                            if !proc_handle.is_null() {
                                if TerminateProcess(proc_handle, 0) != 0 {
                                    let _ = WaitForSingleObject(proc_handle, 1500);
                                    killed += 1;
                                }
                                let _ = CloseHandle(proc_handle);
                            }
                        }
                    }
                    if Process32NextW(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snap);
        }
        killed
    }
    #[cfg(not(windows))]
    {
        0
    }
}

fn main() -> Result<()> {
    cleanup_stale_update_files();
    let killed_prev_sync = replace_previous_sync_processes();
    if killed_prev_sync > 0 {
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = AppConfig::load_or_default();
    let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
    let headless = std::env::args().any(|a| a == "--headless");

    info!(
        "NFC Bridge Sync v{} ({}) | IP={} | PC={} | Modo={}",
        env!("CARGO_PKG_VERSION"),
        env!("NFC_BRIDGE_BUILD_ID"),
        local_ip,
        config.pc_name,
        if headless { "headless-http" } else { "desktop-native" }
    );

    info!("Directorio de datos: {:?}", config.data_dir);
    info!(
        "Interfaz Stitch integrada: {} KB incrustados en .exe",
        (EmbeddedUi::INDEX_HTML.len() + EmbeddedUi::TRAY_HTML.len() + EmbeddedUi::CASES_HTML.len()) / 1024
    );

    let driver_mgr = DriverManager::new();
    let mut status = driver_mgr.check_driver_status();

    if status == DriverStatus::InstalledStopped {
        info!("Servicio NFC Bridge Driver instalado pero detenido. Intentando iniciar...");
        let mut sc = std::process::Command::new("sc.exe");
        sc.args(["start", "NfcBridgeDriver"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            sc.creation_flags(0x0800_0000);
        }
        let _ = sc.output();
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

    let rt = tokio::runtime::Runtime::new()?;
    let ipc_client = DriverIpcClient::new();
    let updater = DesktopUpdater::new();
    rt.block_on(async {
        updater
            .set_product_identity("sync", "NFC Bridge Sync", env!("CARGO_PKG_VERSION"))
            .await;
    });

    let rt_handle = rt.handle().clone();
    let ipc_for_ui = ipc_client.clone();
    let upd_for_ui = updater.clone();
    let data_dir = config.data_dir.clone();
    let runtime_handle = std::thread::spawn(move || {
        rt.block_on(async move {
            if headless {
                let ui_server = DesktopUiServer::with_updater(
                    ipc_client.clone(),
                    updater.clone(),
                    DEFAULT_UI_HTTP_PORT,
                );
                tokio::spawn(async move {
                    ui_server.run().await;
                });
            }

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

    if headless {
        let _ = runtime_handle.join();
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        nfc_bridge_sync::egui_app::run_desktop_ui(
            "NFC Bridge Sync",
            ipc_for_ui,
            upd_for_ui,
            rt_handle,
            data_dir,
        )?;
    }

    Ok(())
}
