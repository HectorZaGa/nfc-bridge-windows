pub mod config;
pub mod driver_manager;
pub mod server;
pub mod ui_http;
pub mod updater;

pub use config::{AppConfig, EmbeddedUi};
pub use driver_manager::{DriverManager, DriverStatus};
pub use server::{DesktopLiveState, DriverIpcClient};
pub use ui_http::DesktopUiServer;
pub use updater::DesktopUpdater;
