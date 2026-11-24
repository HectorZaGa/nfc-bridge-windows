//! # Configuración y Modo Portable
//!
//! Detecta automáticamente si la aplicación se está ejecutando en modo portable
//! (archivo junto al ejecutable o memoria USB) o en modo instalación estándar (%APPDATA%).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub is_portable: bool,
    pub data_dir: PathBuf,
    pub pc_name: String,
    pub tcp_port: u16,
    pub auto_usb_priority: bool,
    pub autostart_with_windows: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let (is_portable, data_dir) = resolve_data_directory();
        let pc_name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Master PC Race".to_string());

        Self {
            is_portable,
            data_dir,
            pc_name,
            tcp_port: 45362,
            auto_usb_priority: true,
            autostart_with_windows: false,
        }
    }
}

/// Determina si estamos en modo portable o en AppData de Windows.
fn resolve_data_directory() -> (bool, PathBuf) {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // Si existe una marca "portable.dat" o "nfc-bridge-portable" junto al .exe
            let portable_marker = exe_dir.join("portable.dat");
            let local_config = exe_dir.join("nfc-bridge.json");

            if portable_marker.exists() || local_config.exists() {
                info!("Modo PORTABLE activo: los datos se guardarán junto al ejecutable en {:?}", exe_dir);
                return (true, exe_dir.to_path_buf());
            }
        }
    }

    // Modo estándar: %APPDATA%\NFC Bridge
    let app_data = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let standard_dir = app_data.join("NFC Bridge");
    (false, standard_dir)
}

/// Recursos de interfaz de Stitch incrustados directamente dentro del binario compilado (.exe)
pub struct EmbeddedUi;

impl EmbeddedUi {
    pub const INDEX_HTML: &'static str = include_str!("../ui/index.html");
    pub const TRAY_HTML: &'static str = include_str!("../ui/tray.html");
    pub const CASES_HTML: &'static str = include_str!("../ui/cases.html");
}
