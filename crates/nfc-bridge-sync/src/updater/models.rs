use crate::config::is_portable_execution;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GhAsset {
    pub(crate) name: String,
    pub(crate) browser_download_url: String,
    #[serde(default)]
    pub(crate) size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GhRelease {
    pub(crate) tag_name: String,
    #[serde(default)]
    pub(crate) draft: bool,
    #[serde(default)]
    pub(crate) assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct ReleaseAssetMatch {
    pub version: String,
    pub tag_name: String,
    pub asset_name: String,
    pub download_url: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpdatesOverviewState {
    pub app_product: String,
    pub app_display_name: String,
    pub is_portable: bool,
    pub app_version: String,
    pub app_status: String,
    pub app_message: String,
    pub app_available: Option<ReleaseAssetMatch>,
    pub pending_restart: bool,
    pub restart_exe_path: Option<String>,
    pub driver_version: Option<String>,
    pub driver_status: String,
    pub driver_message: String,
    pub driver_available: Option<ReleaseAssetMatch>,
}

impl UpdatesOverviewState {
    pub fn for_current_binary() -> Self {
        let product = detect_running_product_slug();
        let display_name = if product == "tools" {
            "NFC Bridge Tools".to_string()
        } else {
            "NFC Bridge Sync".to_string()
        };
        let (is_portable, _) = is_portable_execution();
        Self {
            app_product: product,
            app_display_name: display_name,
            is_portable,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            app_status: "idle".to_string(),
            app_message: "Sin comprobar aún".to_string(),
            app_available: None,
            pending_restart: false,
            restart_exe_path: None,
            driver_version: None,
            driver_status: "idle".to_string(),
            driver_message: "Sin comprobar aún".to_string(),
            driver_available: None,
        }
    }
}

pub fn detect_running_product_slug() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(name) = exe.file_name().and_then(|n| n.to_str()) {
            let lower = name.to_ascii_lowercase();
            if lower.contains("tools") {
                return "tools".to_string();
            }
        }
    }
    "sync".to_string()
}
