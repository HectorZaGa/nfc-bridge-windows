use anyhow::Result;
use core_proto::{FrameCodec, MessageType, NfcBridgeFrame};
use nfc_bridge_sync::ui_http::DEFAULT_UI_HTTP_PORT;
use nfc_bridge_sync::updater::cleanup_stale_update_files;
use nfc_bridge_sync::{
    DesktopUiServer, DesktopUpdater, DriverIpcClient, DriverManager, DriverStatus,
};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    cleanup_stale_update_files();
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let is_portable = cfg!(feature = "portable") || std::env::current_exe()
        .ok()
        .map(|p| {
            let in_dir = p.parent().map(|dir| dir.join("portable.dat").exists() || dir.join("nfc-bridge.json").exists()).unwrap_or(false);
            let in_name = p.file_name().and_then(|n| n.to_str()).map(|n| {
                let lower = n.to_lowercase();
                lower.contains("portable") || lower.ends_with("p.exe")
            }).unwrap_or(false);
            in_dir || in_name
        })
        .unwrap_or(false);

    println!("==================================================");
    println!(
        "   NFC Bridge Tools - Windows Pro v{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("NFC_BRIDGE_BUILD_ID")
    );
    if is_portable {
        println!("             [ MODO PORTABLE ACTIVO ]             ");
    }
    println!("==================================================");
    println!(" [1] Monitor de APDUs en tiempo real");
    println!(" [2] Comprobar lectores PC/SC del sistema");
    println!(" [3] Inyector / Emulador de respuestas APDU");
    println!(" [4] Diagnóstico de latencia USB y red local");
    println!("==================================================");

    info!("Iniciando suite de herramientas avanzadas...");
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
        let _ = std::process::Command::new("sc.exe")
            .args(["start", "NfcBridgeDriver"])
            .output();
    } else if status == DriverStatus::NotInstalled {
        if let Some(local_driver) = driver_mgr.find_local_driver_binary() {
            let _ = driver_mgr.start_local_driver_background(&local_driver);
        } else {
            warn!("Driver no detectado para NFC Bridge Tools.");
        }
    }

    let ipc_client = DriverIpcClient::new();
    ipc_client
        .set_product_meta(env!("CARGO_PKG_VERSION"), env!("NFC_BRIDGE_BUILD_ID"))
        .await;
    let updater = DesktopUpdater::new();
    updater
        .set_product_identity("tools", "NFC Bridge Tools", env!("CARGO_PKG_VERSION"))
        .await;
    let ui_server = DesktopUiServer::with_updater(
        ipc_client.clone(),
        updater.clone(),
        DEFAULT_UI_HTTP_PORT,
    );
    tokio::spawn(async move {
        ui_server.run().await;
    });

    {
        let updater_init = updater.clone();
        let ipc_init = ipc_client.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
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

    if std::env::args().any(|a| a == "--open-ui" || a == "--ui") {
        let url = format!("http://127.0.0.1:{}", DEFAULT_UI_HTTP_PORT);
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn();
    }

    ipc_client.run_forever().await?;
    Ok(())
}
