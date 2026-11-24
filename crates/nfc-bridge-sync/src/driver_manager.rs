//! # Driver Manager
//!
//! Verifica la presencia del controlador virtual PC/SC en Windows y permite
//! consultar y descargar automáticamente la última versión desde el repositorio
//! `HectorZaGa/nfc-bridge-driver`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

pub const DEFAULT_DRIVER_REPO_OWNER: &str = "HectorZaGa";
pub const DEFAULT_DRIVER_REPO_NAME: &str = "nfc-bridge-driver";
pub const DRIVER_NAMED_PIPE: &str = r"\\.\pipe\nfc_bridge_driver";
pub const ALT_DRIVER_NAMED_PIPE: &str = r"\\.\pipe\NfcBridgeDriver";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub published_at: Option<String>,
    pub assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DriverStatus {
    /// El driver está instalado y el canal IPC (Named Pipe) responde
    Running,
    /// El servicio/driver parece instalado pero el Pipe no está abierto
    InstalledStopped,
    /// El driver no se encuentra instalado en Windows
    NotInstalled,
}

pub struct DriverManager {
    repo_owner: String,
    repo_name: String,
    client: reqwest::Client,
}

impl DriverManager {
    pub fn new() -> Self {
        Self {
            repo_owner: DEFAULT_DRIVER_REPO_OWNER.to_string(),
            repo_name: DEFAULT_DRIVER_REPO_NAME.to_string(),
            client: reqwest::Client::builder()
                .user_agent("nfc-bridge-sync/0.1.0")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Comprueba si el Named Pipe del driver virtual está activo y respondiendo.
    pub fn check_driver_status(&self) -> DriverStatus {
        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;
            let pipes = [DRIVER_NAMED_PIPE, ALT_DRIVER_NAMED_PIPE];

            for pipe in pipes {
                match OpenOptions::new().read(true).write(true).open(pipe) {
                    Ok(_) => return DriverStatus::Running,
                    Err(e) => {
                        let raw_code = e.raw_os_error().unwrap_or(0);
                        // ERROR_PIPE_BUSY (231) indica que el Named Pipe del driver sí existe y está activo
                        if raw_code == 231 {
                            return DriverStatus::Running;
                        }
                    }
                }
            }

            // Si el pipe no está abierto, verificar si el servicio está instalado en Windows
            if let Ok(out) = std::process::Command::new("sc.exe")
                .args(["query", "NfcBridgeDriver"])
                .output()
            {
                if out.status.success() {
                    return DriverStatus::InstalledStopped;
                }
            }

            DriverStatus::NotInstalled
        }
        #[cfg(not(target_os = "windows"))]
        {
            DriverStatus::NotInstalled
        }
    }

    /// Consulta en la API de GitHub la versión más reciente publicada de nfc-bridge-driver.
    pub async fn fetch_latest_release(&self) -> Result<GitHubRelease> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.repo_owner, self.repo_name
        );
        info!("Consultando última versión del driver en: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Fallo de red al consultar releases en GitHub")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "La API de GitHub retornó código de estado: {}",
                response.status()
            );
        }

        let release = response
            .json::<GitHubRelease>()
            .await
            .context("Error al deserializar la información de la release")?;

        info!("Última release encontrada: {}", release.tag_name);
        Ok(release)
    }

    /// Descarga un archivo de release del driver al destino especificado.
    pub async fn download_driver_asset(
        &self,
        asset_url: &str,
        destination_dir: &Path,
        file_name: &str,
    ) -> Result<PathBuf> {
        tokio::fs::create_dir_all(destination_dir).await?;
        let target_path = destination_dir.join(file_name);

        info!("Descargando driver desde {} hacia {:?}", asset_url, target_path);

        let response = self
            .client
            .get(asset_url)
            .send()
            .await
            .context("Error al solicitar descarga del archivo del driver")?;

        if !response.status().is_success() {
            anyhow::bail!("Error de descarga con status: {}", response.status());
        }

        let content = response
            .bytes()
            .await
            .context("Error al leer el cuerpo del archivo del driver")?;

        tokio::fs::write(&target_path, &content)
            .await
            .context("Error al escribir el instalador del driver en disco")?;

        info!("Descarga completada: {:?}", target_path);
        Ok(target_path)
    }

    /// Busca si el binario del driver se encuentra junto a la aplicación (útil para paquetes portables).
    pub fn find_local_driver_binary(&self) -> Option<PathBuf> {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                let candidates = [
                    parent.join("nfc-bridge-driver.exe"),
                    parent.join("driver").join("nfc-bridge-driver.exe"),
                    parent.join("..").join("driver").join("nfc-bridge-driver.exe"),
                ];
                for candidate in candidates {
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// Inicia el driver virtual silenciosamente en segundo plano.
    pub fn start_local_driver_background(&self, driver_exe: &Path) -> std::io::Result<std::process::Child> {
        info!("Iniciando driver virtual silencioso desde {:?}", driver_exe);
        std::process::Command::new(driver_exe).spawn()
    }

    /// Registra e inicia el driver virtual como Servicio de Windows (solicita permisos de administrador).
    pub fn install_driver_service_elevated(&self, driver_exe: &Path) -> std::io::Result<bool> {
        let bin_path = format!("\"{}\"", driver_exe.to_string_lossy());
        let script = format!(
            "Start-Process sc.exe -ArgumentList 'create NfcBridgeDriver binPath= \"{}\" start= auto DisplayName= \"NFC Bridge Driver Service\"' -Verb RunAs -Wait; Start-Process sc.exe -ArgumentList 'start NfcBridgeDriver' -Verb RunAs -Wait",
            bin_path
        );

        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
            .status()?;

        Ok(status.success())
    }
}
