pub mod bootstrap;
pub mod config;
pub mod driver_manager;
pub mod i18n;
pub mod ipc;
pub mod platform;
#[cfg(target_os = "windows")]
pub mod ui;
#[cfg(feature = "headless-http")]
pub mod ui_http;
pub mod updater;

pub use config::{AppConfig, EmbeddedI18n};
#[cfg(feature = "headless-http")]
pub use config::EmbeddedUi;
pub use driver_manager::{DriverManager, DriverStatus};
pub use ipc::{DesktopLiveState, DriverIpcClient};
#[cfg(feature = "headless-http")]
pub use ui_http::DesktopUiServer;
pub use updater::DesktopUpdater;
