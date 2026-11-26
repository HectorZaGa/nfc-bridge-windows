pub mod bootstrap;
pub mod config;
pub mod driver_manager;
pub mod i18n;
pub mod ipc;
pub mod platform;
#[cfg(target_os = "windows")]
pub mod ui;
pub mod updater;

pub use config::{AppConfig, EmbeddedI18n};
pub use driver_manager::{DriverManager, DriverStatus};
pub use ipc::{DesktopLiveState, DriverIpcClient};
pub use updater::DesktopUpdater;
