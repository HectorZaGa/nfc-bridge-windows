//! # Configuración y Modo Portable Inmune a Renombrado
//!
//! Detecta automáticamente si la aplicación se está ejecutando en modo portable o instalado.
//!
//! ### Detección Infalible de Modo Portable:
//! 1. **Bandera de compilación (Incrustada en código máquina):** Si el binario fue compilado
//!    con `--features portable`, siempre será portable aunque el usuario le cambie el nombre
//!    a cualquier cosa (`sync.exe`, `app.exe`, etc.).
//! 2. **Marcador de carpeta (Portable Container):** Si existe `portable.dat`, `.portable` o
//!    `nfc-bridge.json` en la misma carpeta que el ejecutable.
//! 3. **Heurística de nombre de archivo:** Si el nombre del ejecutable contiene `-portable.exe`,
//!    `portable` o termina en `p.exe`.

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
                    // Mantener el data_dir y flag portable resueltos para la instancia actual
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
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));

    // 1. Detección por feature incrustada en compilación (Inmune a cambio de nombre del archivo)
    if cfg!(feature = "portable") {
        return (true, exe_dir);
    }

    // 2. Detección por marcadores locales en la carpeta del ejecutable
    if let Some(ref dir) = exe_dir {
        if dir.join("portable.dat").exists()
            || dir.join(".portable").exists()
            || dir.join("nfc-bridge.json").exists()
        {
            return (true, exe_dir);
        }
    }

    // 3. Detección heurística por nombre de archivo (ej. *-portable.exe, sync-portable.exe)
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

/// Resuelve el directorio de datos (%APPDATA% o junto al .exe si es portable).
fn resolve_data_directory() -> (bool, PathBuf) {
    let (is_portable, exe_dir) = is_portable_execution();

    if is_portable {
        if let Some(dir) = exe_dir {
            // Asegurar creación del marcador portable.dat para persistencia
            let marker = dir.join("portable.dat");
            if !marker.exists() {
                let _ = std::fs::write(&marker, "NFC Bridge Portable Marker\n");
            }
            info!("Modo PORTABLE activo: los datos se guardarán junto al ejecutable en {:?}", dir);
            return (true, dir);
        }
    }

    // Modo estándar de instalación en Windows: %APPDATA%\NFC Bridge
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
