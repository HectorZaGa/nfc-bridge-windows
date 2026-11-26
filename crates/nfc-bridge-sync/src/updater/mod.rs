//! Gestor de actualizaciones desde GitHub Releases para NFC Bridge Desktop (Sync / Tools) y Driver.

pub mod install;
pub mod models;
pub mod version;

pub use install::cleanup_stale_update_files;
pub use models::{ReleaseAssetMatch, UpdatesOverviewState};
pub use version::{compare_semver, normalize_base_version};

use anyhow::{Context, Result};
use install::{
    apply_portable_archive_update, default_updates_cache_dir, extract_exe_from_zip,
    launch_detached, stop_named_driver_processes,
};
use models::GhRelease;
use version::find_best_release_match;
use crate::driver_manager::{DriverManager, DriverStatus};
use crate::platform::launch_elevated_or_normal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub const WINDOWS_REPO: &str = "HectorZaGa/nfc-bridge-windows";
pub const DRIVER_REPO: &str = "HectorZaGa/nfc-bridge-driver";

#[derive(Clone)]
pub struct DesktopUpdater {
    client: reqwest::Client,
    state: Arc<RwLock<UpdatesOverviewState>>,
}

impl Default for DesktopUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopUpdater {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("NFC-Bridge-Windows-Updater")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self {
            client,
            state: Arc::new(RwLock::new(UpdatesOverviewState::for_current_binary())),
        }
    }

    pub async fn set_product_identity(&self, product_slug: &str, display_name: &str, version: &str) {
        let mut st = self.state.write().await;
        st.app_product = product_slug.to_string();
        st.app_display_name = display_name.to_string();
        st.app_version = version.to_string();
    }

    pub async fn set_known_driver_version(&self, version: Option<String>) {
        let mut st = self.state.write().await;
        if version.is_some() {
            st.driver_version = version;
        }
    }

    pub async fn snapshot(&self) -> UpdatesOverviewState {
        self.state.read().await.clone()
    }

    pub async fn check_all(
        &self,
        known_driver_version: Option<String>,
        auto_update_app: bool,
        auto_update_driver: bool,
    ) {
        {
            let mut st = self.state.write().await;
            if known_driver_version.is_some() {
                st.driver_version = known_driver_version;
            }
            if st.app_status != "downloading" && st.app_status != "installed_restart" {
                st.app_status = "checking".to_string();
                st.app_message = "Consultando GitHub Releases...".to_string();
            }
            if st.driver_status != "downloading" && st.driver_status != "installed" {
                st.driver_status = "checking".to_string();
                st.driver_message = "Consultando GitHub Releases...".to_string();
            }
        }

        let cur = self.snapshot().await;

        match self
            .fetch_matching_release(
                WINDOWS_REPO,
                &format!("nfc-bridge-{}-", cur.app_product),
                cur.is_portable,
            )
            .await
        {
            Ok(Some(matched)) => {
                if compare_semver(&matched.version, &cur.app_version) > 0 {
                    let msg = format!("Nueva versión v{} ({})", matched.version, matched.asset_name);
                    {
                        let mut st = self.state.write().await;
                        st.app_status = "available".to_string();
                        st.app_message = msg;
                        st.app_available = Some(matched);
                    }
                    if auto_update_app {
                        let _ = self.install_app_update().await;
                    }
                } else {
                    let mut st = self.state.write().await;
                    if st.app_status != "installed_restart" {
                        st.app_status = "up_to_date".to_string();
                        st.app_message = format!("Actualizado (v{})", normalize_base_version(&cur.app_version));
                        st.app_available = None;
                    }
                }
            }
            Ok(None) => {
                let mut st = self.state.write().await;
                if st.app_status != "installed_restart" {
                    st.app_status = "no_releases".to_string();
                    st.app_message = format!("Sin lanzamientos publicados aún para {}", cur.app_display_name);
                    st.app_available = None;
                }
            }
            Err(e) => {
                let mut st = self.state.write().await;
                if st.app_status != "installed_restart" {
                    st.app_status = "error".to_string();
                    st.app_message = format!("No se pudo verificar: {}", e);
                }
            }
        }

        let local_drv = cur.driver_version.clone().unwrap_or_else(|| "0.0.0".to_string());
        let drv_missing = DriverManager::new().check_driver_status() == DriverStatus::NotInstalled;
        match self
            .fetch_matching_release(DRIVER_REPO, "nfc-bridge-driver-", false)
            .await
        {
            Ok(Some(matched)) => {
                let is_newer = cur.driver_version.is_none()
                    || compare_semver(&matched.version, &local_drv) > 0;
                if is_newer {
                    let msg = if cur.driver_version.is_some() {
                        format!("Nueva versión v{} ({})", matched.version, matched.asset_name)
                    } else {
                        format!("Disponible v{} ({})", matched.version, matched.asset_name)
                    };
                    {
                        let mut st = self.state.write().await;
                        st.driver_status = "available".to_string();
                        st.driver_message = msg;
                        st.driver_available = Some(matched);
                    }
                    if !drv_missing && auto_update_driver {
                        let _ = self.install_driver_internal(false).await;
                    }
                } else {
                    let mut st = self.state.write().await;
                    if drv_missing {
                        st.driver_status = "not_installed".to_string();
                        st.driver_message =
                            "NFC Bridge Driver no está instalado en este equipo.".to_string();
                    } else {
                        st.driver_status = "up_to_date".to_string();
                        st.driver_message =
                            format!("Actualizado (v{})", normalize_base_version(&local_drv));
                    }
                    st.driver_available = None;
                }
            }
            Ok(None) => {
                let mut st = self.state.write().await;
                if drv_missing {
                    st.driver_status = "not_installed".to_string();
                    st.driver_message =
                        "NFC Bridge Driver no está instalado en este equipo.".to_string();
                } else {
                    st.driver_status = "no_releases".to_string();
                    st.driver_message = "Sin lanzamientos publicados aún en GitHub".to_string();
                }
                st.driver_available = None;
            }
            Err(e) => {
                let mut st = self.state.write().await;
                if drv_missing {
                    st.driver_status = "not_installed".to_string();
                    st.driver_message =
                        "NFC Bridge Driver no está instalado en este equipo.".to_string();
                } else {
                    st.driver_status = "error".to_string();
                    st.driver_message = format!("No se pudo verificar: {}", e);
                }
            }
        }
    }

    async fn fetch_matching_release(
        &self,
        repo: &str,
        asset_prefix: &str,
        prefer_portable_zip: bool,
    ) -> Result<Option<ReleaseAssetMatch>> {
        let url = format!("https://api.github.com/repos/{}/releases?per_page=25", repo);
        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .context("Error de conexión con GitHub")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            anyhow::bail!("HTTP {}", resp.status());
        }

        let releases = resp
            .json::<Vec<GhRelease>>()
            .await
            .context("Respuesta JSON inválida")?;

        Ok(find_best_release_match(
            &releases,
            asset_prefix,
            prefer_portable_zip,
        ))
    }

    async fn download_to_path(&self, url: &str, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let resp = self
            .client
            .get(url)
            .header("Accept", "application/octet-stream")
            .send()
            .await
            .context("Error descargando archivo")?;
        if !resp.status().is_success() {
            anyhow::bail!("HTTP {} al descargar", resp.status());
        }
        let bytes = resp.bytes().await.context("Error leyendo bytes de descarga")?;
        if bytes.is_empty() {
            anyhow::bail!("Archivo descargado vacío");
        }
        tokio::fs::write(dest, &bytes).await?;
        Ok(())
    }

    pub async fn install_app_update(&self) -> Result<()> {
        let (is_portable, release) = {
            let mut st = self.state.write().await;
            let Some(rel) = st.app_available.clone() else {
                anyhow::bail!("No hay actualización disponible para el programa");
            };
            st.app_status = "downloading".to_string();
            st.app_message = format!("Descargando {}...", rel.asset_name);
            (st.is_portable, rel)
        };

        let exe_path = std::env::current_exe().context("No se pudo resolver ruta del ejecutable")?;
        let exe_dir = exe_path
            .parent()
            .context("Sin directorio padre")?
            .to_path_buf();

        if is_portable {
            let archive_path = exe_dir.join(&release.asset_name);
            if let Err(e) = self.download_to_path(&release.download_url, &archive_path).await {
                let mut st = self.state.write().await;
                st.app_status = "error".to_string();
                st.app_message = format!("Error descargando actualización: {}", e);
                return Err(e);
            }

            let lower = release.asset_name.to_ascii_lowercase();
            let install_res = if lower.ends_with(".zip") {
                apply_portable_archive_update(&exe_path, &archive_path)
            } else {
                Ok(archive_path)
            };
            match install_res {
                Ok(target_launch_exe) => {
                    info!(
                        "Actualización portable aplicada en {:?}. Listo para reiniciar.",
                        target_launch_exe
                    );
                    let mut st = self.state.write().await;
                    st.app_status = "installed_restart".to_string();
                    st.app_message = format!("v{} lista junto al ejecutable. Reinicia para aplicar.", release.version);
                    st.pending_restart = true;
                    st.restart_exe_path = Some(target_launch_exe.to_string_lossy().to_string());
                    Ok(())
                }
                Err(e) => {
                    let mut st = self.state.write().await;
                    st.app_status = "error".to_string();
                    st.app_message = format!("Error extrayendo portable: {}", e);
                    Err(e)
                }
            }
        } else {
            let updates_dir = default_updates_cache_dir();
            let installer_path = updates_dir.join(&release.asset_name);
            if let Err(e) = self.download_to_path(&release.download_url, &installer_path).await {
                let mut st = self.state.write().await;
                st.app_status = "error".to_string();
                st.app_message = format!("Error descargando instalador: {}", e);
                return Err(e);
            }

            if release.asset_name.to_ascii_lowercase().ends_with(".zip") {
                let extracted_exe = extract_exe_from_zip(&installer_path, &updates_dir.join("app_stage"))?;
                launch_detached(&extracted_exe, &[])?;
            } else {
                launch_detached(
                    &installer_path,
                    &["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"],
                )?;
            }
            let mut st = self.state.write().await;
            st.app_status = "installed_external".to_string();
            st.app_message = format!("Instalador v{} iniciado.", release.version);
            Ok(())
        }
    }

    pub async fn install_driver_update(&self) -> Result<()> {
        self.install_or_update_driver(true).await
    }

    pub async fn install_or_update_driver(&self, interactive: bool) -> Result<()> {
        let has_avail = self.state.read().await.driver_available.is_some();
        if !has_avail {
            if let Ok(Some(matched)) = self
                .fetch_matching_release(DRIVER_REPO, "nfc-bridge-driver-", false)
                .await
            {
                let mut st = self.state.write().await;
                st.driver_available = Some(matched);
            } else {
                let mut st = self.state.write().await;
                st.driver_status = "no_releases".to_string();
                st.driver_message =
                    "No se encontró un instalador compatible publicado en GitHub Releases."
                        .to_string();
                anyhow::bail!("Sin instalador de driver disponible en GitHub Releases");
            }
        }
        self.install_driver_internal(interactive).await
    }

    async fn install_driver_internal(&self, interactive: bool) -> Result<()> {
        let release = {
            let mut st = self.state.write().await;
            let Some(rel) = st.driver_available.clone() else {
                anyhow::bail!("No hay actualización disponible para el driver");
            };
            st.driver_status = "downloading".to_string();
            st.driver_message = format!("Descargando {}...", rel.asset_name);
            rel
        };

        // El controlador se instala como servicio de Windows; el instalador descargado
        // siempre va a la caché temporal de AppData para no ensuciar carpetas portables.
        let dest_dir = default_updates_cache_dir();
        let asset_path = dest_dir.join(&release.asset_name);

        if let Err(e) = self.download_to_path(&release.download_url, &asset_path).await {
            let mut st = self.state.write().await;
            st.driver_status = "error".to_string();
            st.driver_message = format!("Error descargando driver: {}", e);
            return Err(e);
        }

        let lower = release.asset_name.to_ascii_lowercase();
        let target_exe = if lower.ends_with(".zip") {
            let stage = dest_dir.join(".nb_drv_tmp");
            let exe = extract_exe_from_zip(&asset_path, &stage)?;
            let final_drv = dest_dir.join("nfc-bridge-driver.exe");
            let _ = stop_named_driver_processes();
            if final_drv.exists() {
                let old_drv = dest_dir.join("nfc-bridge-driver.old.exe");
                let _ = std::fs::remove_file(&old_drv);
                let _ = std::fs::rename(&final_drv, &old_drv);
            }
            std::fs::copy(&exe, &final_drv)?;
            let _ = std::fs::remove_dir_all(&stage);
            final_drv
        } else {
            asset_path
        };

        let launch_res = if lower.contains("-setup") || lower.contains("-installer") {
            if interactive {
                launch_elevated_or_normal(&target_exe, &[])
            } else {
                launch_elevated_or_normal(
                    &target_exe,
                    &["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"],
                )
            }
        } else {
            launch_elevated_or_normal(&target_exe, &["--install-service"])
        };

        if let Err(e) = launch_res {
            let mut st = self.state.write().await;
            st.driver_status = "available".to_string();
            st.driver_message = format!("Disponible v{} ({})", release.version, release.asset_name);
            return Err(e);
        }

        let mut st = self.state.write().await;
        st.driver_status = "installed".to_string();
        st.driver_message = format!("Driver v{} instalado y reiniciado.", release.version);
        st.driver_available = None;
        Ok(())
    }

    pub async fn restart_running_app(&self) -> Result<()> {
        let exe_to_run = {
            let st = self.state.read().await;
            st.restart_exe_path
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_exe().unwrap_or_default())
        };
        if !exe_to_run.exists() {
            anyhow::bail!("No se encontró el ejecutable para reiniciar");
        }
        launch_detached(&exe_to_run, &["--open-ui"])?;
        tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            std::process::exit(0);
        });
        Ok(())
    }
}
