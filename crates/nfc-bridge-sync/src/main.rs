#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use nfc_bridge_sync::bootstrap::{run_desktop_app, DesktopLaunchSpec};
use nfc_bridge_sync::AppConfig;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = AppConfig::load_or_default();
    run_desktop_app(
        DesktopLaunchSpec {
            product_slug: "sync",
            display_name: "NFC Bridge Sync",
            version: env!("CARGO_PKG_VERSION"),
            build_id: env!("NFC_BRIDGE_BUILD_ID"),
            single_instance_sync: true,
            sync_product_meta_to_ipc: false,
        },
        config,
    )
}
