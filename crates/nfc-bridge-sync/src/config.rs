//! # Configuración y Modo Portable Inmune a Renombrado
//!
//! Detecta automáticamente si la aplicación se ejecuta en modo portable o instalado
//! y centraliza las rutas de datos y preferencias de interfaz.

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

impl AppConfig {
    /// Carga la configuración guardada en disco o devuelve los valores predeterminados.
    pub fn load_or_default() -> Self {
        let default = Self::default();
        let config_file = default.data_dir.join("nfc-bridge.json");

        if config_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_file) {
                if let Ok(mut loaded) = serde_json::from_str::<AppConfig>(&content) {
                    loaded.is_portable = default.is_portable;
                    loaded.data_dir = default.data_dir;
                    return loaded;
                }
            }
        }

        default
    }

    /// Guarda la configuración en formato JSON en el directorio de datos.
    pub fn save(&self) -> std::io::Result<()> {
        if !self.data_dir.exists() {
            std::fs::create_dir_all(&self.data_dir)?;
        }
        let config_file = self.data_dir.join("nfc-bridge.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(config_file, json)?;
        Ok(())
    }
}

/// Determina de forma infalible si el binario se ejecuta en modo portable.
pub fn is_portable_execution() -> (bool, Option<PathBuf>) {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if cfg!(feature = "portable") {
        return (true, exe_dir);
    }

    if let Some(ref dir) = exe_dir {
        if dir.join("portable.dat").exists()
            || dir.join(".portable").exists()
            || dir.join("nfc-bridge.json").exists()
        {
            return (true, exe_dir);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(name) = exe_path.file_name().and_then(|n| n.to_str()) {
            let lower = name.to_lowercase();
            if lower.contains("portable") || lower.ends_with("p.exe") {
                return (true, exe_dir);
            }
        }
    }

    (false, exe_dir)
}

pub fn installed_app_data_dir() -> PathBuf {
    std::env::var("APPDATA")
        .or_else(|_| std::env::var("LOCALAPPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("NFC Bridge")
}

/// Resuelve el directorio de datos (%APPDATA%\NFC Bridge o junto al .exe si es portable).
pub fn resolve_data_directory() -> (bool, PathBuf) {
    let (is_portable, exe_dir) = is_portable_execution();

    if is_portable {
        if let Some(dir) = exe_dir {
            let marker = dir.join("portable.dat");
            if !marker.exists() {
                let _ = std::fs::write(&marker, "NFC Bridge Portable Marker\n");
            }
            info!(
                "Modo PORTABLE activo: los datos se guardarán junto al ejecutable en {:?}",
                dir
            );
            return (true, dir);
        }
    }

    (false, installed_app_data_dir())
}

/// Ruta activa para `sync_ui_settings.json` respetando modo portable e instalado.
pub fn ui_settings_path() -> PathBuf {
    let (is_portable, exe_dir) = is_portable_execution();
    if is_portable {
        if let Some(dir) = exe_dir {
            let local_settings = dir.join("sync_ui_settings.json");
            if !local_settings.exists() {
                let legacy = installed_app_data_dir().join("sync_ui_settings.json");
                if legacy.exists() {
                    let _ = std::fs::copy(&legacy, &local_settings);
                }
            }
            return local_settings;
        }
    }
    installed_app_data_dir().join("sync_ui_settings.json")
}

/// Diccionarios de internacionalización incrustados (únicamente ~22 KB frente a los 262 KB del HTML legado).
pub struct EmbeddedI18n;

impl EmbeddedI18n {
    pub const EN_JSON: &'static str = include_str!("../ui/i18n/en.json");
    pub const ES_JSON: &'static str = include_str!("../ui/i18n/es.json");
}

#[cfg(feature = "headless-http")]
pub struct EmbeddedUi;

#[cfg(feature = "headless-http")]
impl EmbeddedUi {
    pub const INDEX_HTML: &'static str = include_str!("../ui/index.html");
    pub const TRAY_HTML: &'static str = include_str!("../ui/tray.html");
    pub const CASES_HTML: &'static str = include_str!("../ui/cases.html");
    pub const EN_I18N_JSON: &'static str = EmbeddedI18n::EN_JSON;
    pub const ES_I18N_JSON: &'static str = EmbeddedI18n::ES_JSON;
}
