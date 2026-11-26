//! # Driver IPC Client (Named Pipe)
//!
//! `nfc-bridge-sync` delega toda la conectividad móvil y emulación PC/SC en `nfc-bridge-driver`
//! a través de `\\.\pipe\nfc_bridge_driver`.

mod client;
mod models;
mod uri;

pub use client::DriverIpcClient;
pub use models::{
    load_ui_settings, save_ui_settings, DesktopLiveState, InvalidPairingNoticeState, IpcCommand,
    IpcEvent, IssuedQrData, PendingMobileSwitchState, SyncUiSettings, TransportSnapshotData,
    TrustedDeviceInfo,
};
pub use uri::{build_wifi_direct_qr_uri, normalize_hex_color};
