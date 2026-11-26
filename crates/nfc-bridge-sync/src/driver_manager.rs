//! # Driver Manager
//!
//! Verifica la presencia del controlador virtual PC/SC en Windows y permite
//! consultar y descargar automáticamente la última versión desde `HectorZaGa/nfc-bridge-driver`.

use crate::platform::hidden_command;
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
    /// El driver está instalado y el canal IPC (Named Pipe) responde.
    Running,
    /// El servicio/driver está instalado pero el Pipe no está abierto.
    InstalledStopped,
    /// El driver no se encuentra instalado en Windows.
    NotInstalled,
}

pub struct DriverManager {
    repo_owner: String,
    repo_name: String,
    client: reqwest::Client,
}

impl Default for DriverManager {
    fn default() -> Self {
        Self::new()
    }
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
            for pipe in [DRIVER_NAMED_PIPE, ALT_DRIVER_NAMED_PIPE] {
                match OpenOptions::new().read(true).write(true).open(pipe) {
                    Ok(_) => return DriverStatus::Running,
                    Err(e) => {
                        if e.raw_os_error().unwrap_or(0) == 231 {
                            return DriverStatus::Running;
                        }
                    }
                }
            }

            if let Ok(out) = hidden_command("sc.exe")
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

    pub fn install_driver_service_elevated(&self, driver_exe: &Path) -> std::io::Result<bool> {
        let escaped_exe = driver_exe.to_string_lossy().replace('\'', "''");
        let script = format!(
            "$p = Start-Process -FilePath '{}' -ArgumentList '--install-service' -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
            escaped_exe
        );

        let status = hidden_command("powershell.exe")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
            .status()?;

        Ok(status.success())
    }
}
