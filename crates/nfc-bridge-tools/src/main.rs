#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use core_proto::{FrameCodec, MessageType, NfcBridgeFrame};
use nfc_bridge_sync::ui_http::DEFAULT_UI_HTTP_PORT;
use nfc_bridge_sync::updater::cleanup_stale_update_files;
use nfc_bridge_sync::{
    AppConfig, DesktopUiServer, DesktopUpdater, DriverIpcClient, DriverManager, DriverStatus,
};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

fn main() -> Result<()> {
    cleanup_stale_update_files();
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = AppConfig::load_or_default();
    let headless = std::env::args().any(|a| a == "--headless");

    info!(
        "NFC Bridge Tools v{} ({}) | Portable={}",
        env!("CARGO_PKG_VERSION"),
        env!("NFC_BRIDGE_BUILD_ID"),
        config.is_portable
    );

    let test_frame = NfcBridgeFrame::new(
        MessageType::ReaderStatus,
        1,
        bytes::Bytes::from_static(b"STATUS:ACTIVE"),
    );
    let encoded = FrameCodec::encode(&test_frame);
    info!("Trama de prueba BAP codificada ({} bytes)", encoded.len());

    if std::env::args().any(|a| a == "--smoke") {
        return Ok(());
    }

    let driver_mgr = DriverManager::new();
    let status = driver_mgr.check_driver_status();
    if status == DriverStatus::InstalledStopped {
        let mut sc = std::process::Command::new("sc.exe");
        sc.args(["start", "NfcBridgeDriver"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            sc.creation_flags(0x0800_0000);
        }
        let _ = sc.output();
    } else if status == DriverStatus::NotInstalled {
        if let Some(local_driver) = driver_mgr.find_local_driver_binary() {
            let _ = driver_mgr.start_local_driver_background(&local_driver);
        } else {
            warn!("Driver no detectado para NFC Bridge Tools.");
        }
    }

    let rt = tokio::runtime::Runtime::new()?;
    let ipc_client = DriverIpcClient::new();
    let updater = DesktopUpdater::new();
    rt.block_on(async {
        ipc_client
            .set_product_meta(env!("CARGO_PKG_VERSION"), env!("NFC_BRIDGE_BUILD_ID"))
            .await;
        updater
            .set_product_identity("tools", "NFC Bridge Tools", env!("CARGO_PKG_VERSION"))
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
        nfc_bridge_sync::desktop_window::run_desktop_ui(
            "NFC Bridge Tools",
            ipc_for_ui,
            upd_for_ui,
            rt_handle,
            data_dir,
        )?;
    }
    Ok(())
}
