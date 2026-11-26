pub mod config;
pub mod driver_manager;
#[cfg(target_os = "windows")]
pub mod egui_app;
#[cfg(target_os = "windows")]
mod egui_i18n;
#[cfg(target_os = "windows")]
mod egui_icons;
#[cfg(target_os = "windows")]
mod egui_qr;
#[cfg(target_os = "windows")]
mod egui_theme;
pub mod server;
pub mod ui_http;
pub mod updater;

pub use config::{AppConfig, EmbeddedUi};
pub use driver_manager::{DriverManager, DriverStatus};
pub use server::{DesktopLiveState, DriverIpcClient};
pub use ui_http::DesktopUiServer;
pub use updater::DesktopUpdater;
