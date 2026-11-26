#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use core_proto::{FrameCodec, MessageType, NfcBridgeFrame};
use nfc_bridge_sync::bootstrap::{run_desktop_app, DesktopLaunchSpec};
use nfc_bridge_sync::AppConfig;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let test_frame = NfcBridgeFrame::new(
        MessageType::ReaderStatus,
        1,
        bytes::Bytes::from_static(b"STATUS:ACTIVE"),
    );
    let encoded = FrameCodec::encode(&test_frame);
    info!("Trama de prueba BAP codificada ({} bytes)", encoded.len());

    if std::env::args().any(|a| a == "--smoke") {
        let config = AppConfig::load_or_default();
        info!(
            "NFC Bridge Tools v{} ({}) | Portable={}",
            env!("CARGO_PKG_VERSION"),
            env!("NFC_BRIDGE_BUILD_ID"),
            config.is_portable
        );
        return Ok(());
    }

    let config = AppConfig::load_or_default();
    run_desktop_app(
        DesktopLaunchSpec {
            product_slug: "tools",
            display_name: "NFC Bridge Tools",
            version: env!("CARGO_PKG_VERSION"),
            build_id: env!("NFC_BRIDGE_BUILD_ID"),
            single_instance_sync: false,
            sync_product_meta_to_ipc: true,
            #[cfg(target_os = "windows")]
            app_ico_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/app.ico")),
            #[cfg(not(target_os = "windows"))]
            app_ico_bytes: &[],
        },
        config,
    )
}
