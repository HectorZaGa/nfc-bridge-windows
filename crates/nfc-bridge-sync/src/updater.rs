//! Gestor de actualizaciones desde GitHub Releases para NFC Bridge Desktop (Sync / Tools) y Driver.
//!
//! - Detecta automáticamente si la instancia en ejecución es Sync o Tools Desktop,
//!   y consulta únicamente los lanzamientos de su propio producto en `HectorZaGa/nfc-bridge-windows`,
//!   más los lanzamientos de `HectorZaGa/nfc-bridge-driver`.
//! - En modo portable, descarga el `.zip` y extrae el binario actualizado directamente en la misma
//!   carpeta donde se ejecuta la aplicación, dejando todo listo para reiniciar.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::is_portable_execution;

pub const WINDOWS_REPO: &str = "HectorZaGa/nfc-bridge-windows";
pub const DRIVER_REPO: &str = "HectorZaGa/nfc-bridge-driver";

#[derive(Debug, Clone, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReleaseAssetMatch {
    pub version: String,
    pub tag_name: String,
    pub asset_name: String,
    pub download_url: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
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

pub fn cleanup_stale_update_files() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.ends_with(".old.exe") || name.ends_with(".exe.old") {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
            let staging = dir.join(".nb_update_tmp");
            if staging.exists() {
                let _ = std::fs::remove_dir_all(&staging);
            }
        }
    }
}

#[derive(Clone)]
pub struct DesktopUpdater {
    client: reqwest::Client,
    state: Arc<RwLock<UpdatesOverviewState>>,
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

        // 1. Verificar programa actual (solo Sync si es Sync, solo Tools si es Tools)
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

        // 2. Verificar driver (HectorZaGa/nfc-bridge-driver) — el driver siempre usa instalador
        let local_drv = cur.driver_version.clone().unwrap_or_else(|| "0.0.0".to_string());
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
                    if auto_update_driver {
                        let _ = self.install_driver_update().await;
                    }
                } else {
                    let mut st = self.state.write().await;
                    st.driver_status = "up_to_date".to_string();
                    st.driver_message = format!("Actualizado (v{})", normalize_base_version(&local_drv));
                    st.driver_available = None;
                }
            }
            Ok(None) => {
                let mut st = self.state.write().await;
                st.driver_status = "no_releases".to_string();
                st.driver_message = "Sin lanzamientos publicados aún en GitHub".to_string();
                st.driver_available = None;
            }
            Err(e) => {
                let mut st = self.state.write().await;
                st.driver_status = "error".to_string();
                st.driver_message = format!("No se pudo verificar: {}", e);
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
            // En modo portable o ejecutable directo, se descarga en la misma carpeta donde corre el programa
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
                launch_detached(&installer_path, &["/UPDATE"])?;
            }
            let mut st = self.state.write().await;
            st.app_status = "installed_external".to_string();
            st.app_message = format!("Instalador v{} iniciado.", release.version);
            Ok(())
        }
    }

    pub async fn install_driver_update(&self) -> Result<()> {
        let (is_portable, release) = {
            let mut st = self.state.write().await;
            let Some(rel) = st.driver_available.clone() else {
                anyhow::bail!("No hay actualización disponible para el driver");
            };
            st.driver_status = "downloading".to_string();
            st.driver_message = format!("Descargando {}...", rel.asset_name);
            (st.is_portable, rel)
        };

        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        let dest_dir = if is_portable {
            exe_dir.unwrap_or_else(default_updates_cache_dir)
        } else {
            default_updates_cache_dir()
        };
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

        if lower.contains("-setup") || lower.contains("-installer") {
            launch_elevated_or_normal(&target_exe, &["/UPDATE"])?;
        } else {
            launch_elevated_or_normal(&target_exe, &["--install-service"])?;
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

fn default_updates_cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("NFCBridge")
        .join("updates")
}

fn find_best_release_match(
    releases: &[GhRelease],
    asset_prefix: &str,
    prefer_portable_zip: bool,
) -> Option<ReleaseAssetMatch> {
    let prefix_lower = asset_prefix.to_ascii_lowercase();
    let mut best: Option<ReleaseAssetMatch> = None;

    for rel in releases {
        if rel.draft {
            continue;
        }
        let candidates: Vec<&GhAsset> = rel
            .assets
            .iter()
            .filter(|a| {
                let l = a.name.to_ascii_lowercase();
                l.starts_with(&prefix_lower) && (l.ends_with(".zip") || l.ends_with(".exe"))
            })
            .collect();
        if candidates.is_empty() {
            continue;
        }

        let chosen = if prefer_portable_zip {
            candidates
                .iter()
                .find(|a| {
                    let l = a.name.to_ascii_lowercase();
                    l.contains("portable") && l.ends_with(".zip")
                })
                .or_else(|| candidates.iter().find(|a| a.name.to_ascii_lowercase().ends_with(".zip")))
                .or_else(|| candidates.first())
                .copied()?
        } else {
            candidates
                .iter()
                .find(|a| {
                    let l = a.name.to_ascii_lowercase();
                    (l.contains("setup") || l.contains("installer")) && l.ends_with(".exe")
                })
                .or_else(|| candidates.iter().find(|a| a.name.to_ascii_lowercase().ends_with(".exe")))
                .or_else(|| candidates.first())
                .copied()?
        };

        let ver = extract_semver_from_names(&chosen.name, &rel.tag_name)?;
        let candidate = ReleaseAssetMatch {
            version: ver,
            tag_name: rel.tag_name.clone(),
            asset_name: chosen.name.clone(),
            download_url: chosen.browser_download_url.clone(),
            size_bytes: chosen.size,
        };
        if best
            .as_ref()
            .is_none_or(|cur| compare_semver(&candidate.version, &cur.version) > 0)
        {
            best = Some(candidate);
        }
    }
    best
}

fn apply_portable_archive_update(current_exe: &Path, archive_path: &Path) -> Result<PathBuf> {
    let exe_dir = current_exe
        .parent()
        .context("Sin carpeta para binario portable")?;
    let stage = exe_dir.join(".nb_update_tmp");
    let extracted_exe = extract_exe_from_zip(archive_path, &stage)?;

    let extracted_name = extracted_exe
        .file_name()
        .context("Nombre inválido en archivo extraído")?;
    let target_exe = exe_dir.join(extracted_name);

    // Copiar otros archivos complementarios del zip (e.g. portable.dat) junto al ejecutable
    if let Ok(entries) = std::fs::read_dir(&stage) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p != extracted_exe {
                if let Some(fname) = p.file_name() {
                    let _ = std::fs::copy(&p, exe_dir.join(fname));
                }
            }
        }
    }

    // Si estamos corriendo desde el mismo nombre (e.g. `NFC Bridge Sync Portable.exe`),
    // Windows permite renombrar el exe en ejecución antes de escribir la nueva versión.
    if current_exe.eq(&target_exe)
        || current_exe
            .file_name()
            .map(|n| n.to_ascii_lowercase())
            == Some(extracted_name.to_ascii_lowercase())
    {
        let backup = current_exe.with_extension("old.exe");
        let _ = std::fs::remove_file(&backup);
        std::fs::rename(current_exe, &backup)
            .context("No se pudo apartar el ejecutable actual en ejecución")?;
        std::fs::copy(&extracted_exe, current_exe)
            .context("No se pudo copiar la nueva versión sobre el ejecutable")?;
        let _ = std::fs::remove_dir_all(&stage);
        Ok(current_exe.to_path_buf())
    } else {
        std::fs::copy(&extracted_exe, &target_exe)
            .context("No se pudo colocar el nuevo ejecutable portable")?;
        let _ = std::fs::remove_dir_all(&stage);
        Ok(target_exe)
    }
}

fn extract_exe_from_zip(zip_path: &Path, stage_dir: &Path) -> Result<PathBuf> {
    if stage_dir.exists() {
        let _ = std::fs::remove_dir_all(stage_dir);
    }
    std::fs::create_dir_all(stage_dir)?;

    let mut extracted_ok = false;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        if let Ok(st) = std::process::Command::new("tar.exe")
            .args([
                "-xf",
                &zip_path.to_string_lossy(),
                "-C",
                &stage_dir.to_string_lossy(),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
        {
            extracted_ok = st.success();
        }
        if !extracted_ok {
            let escaped_zip = zip_path.to_string_lossy().replace('\'', "''");
            let escaped_dst = stage_dir.to_string_lossy().replace('\'', "''");
            let ps_cmd = format!(
                "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                escaped_zip, escaped_dst
            );
            if let Ok(st) = std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
                .creation_flags(CREATE_NO_WINDOW)
                .status()
            {
                extracted_ok = st.success();
            }
        }
    }
    if !extracted_ok {
        anyhow::bail!("No se pudo descomprimir el paquete .zip");
    }

    find_first_exe_recursive(stage_dir).context("El .zip descargado no contiene un archivo .exe")
}

fn find_first_exe_recursive(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_first_exe_recursive(&path) {
                return Some(found);
            }
        }
    }
    None
}

fn launch_detached(exe: &Path, args: &[&str]) -> Result<()> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    cmd.spawn().context("No se pudo iniciar el ejecutable")?;
    Ok(())
}

fn launch_elevated_or_normal(exe: &Path, args: &[&str]) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let escaped_exe = exe.to_string_lossy().replace('\'', "''");
        let args_joined = args
            .iter()
            .map(|a| format!("'{}'", a.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        let script = if args_joined.is_empty() {
            format!("Start-Process -FilePath '{}' -Verb RunAs", escaped_exe)
        } else {
            format!(
                "Start-Process -FilePath '{}' -ArgumentList @({}) -Verb RunAs",
                escaped_exe, args_joined
            )
        };
        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .context("No se pudo elevar permisos de instalación")?;
        if !status.success() {
            warn!("Elevación UAC cancelada o fallida para {:?}", exe);
            anyhow::bail!("Instalación cancelada o sin permisos elevados");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        launch_detached(exe, args)
    }
}

fn stop_named_driver_processes() -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new("taskkill")
            .args(["/IM", "nfc-bridge-driver.exe", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    Ok(())
}

pub fn normalize_base_version(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches('v').trim_start_matches('V');
    for token in trimmed.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let parts: Vec<&str> = token.split('.').filter(|s| !s.is_empty()).collect();
        if parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()) {
            return token.to_string();
        }
    }
    trimmed.to_string()
}

fn extract_semver_from_names(asset_name: &str, tag_name: &str) -> Option<String> {
    let from_asset = extract_triple_digits(asset_name);
    if from_asset.is_some() {
        return from_asset;
    }
    extract_triple_digits(tag_name)
}

fn extract_triple_digits(text: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for chunk in text.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let parts: Vec<&str> = chunk.split('.').collect();
        if parts.len() == 3
            && !parts.iter().any(|p| p.is_empty())
            && parts.iter().all(|p| p.parse::<u32>().is_ok())
        {
            best = Some(chunk.to_string());
            break;
        }
    }
    best
}

pub fn compare_semver(a: &str, b: &str) -> i32 {
    let pa = parse_semver_tuple(a);
    let pb = parse_semver_tuple(b);
    if pa > pb {
        1
    } else if pa < pb {
        -1
    } else {
        0
    }
}

fn parse_semver_tuple(raw: &str) -> (u32, u32, u32) {
    let norm = normalize_base_version(raw);
    let mut it = norm.split('.').map(|s| s.parse::<u32>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_comparison() {
        assert_eq!(compare_semver("0.1.24", "0.1.23"), 1);
        assert_eq!(compare_semver("v1.0.0", "0.9.9"), 1);
        assert_eq!(compare_semver("0.1.23", "0.1.23"), 0);
        assert_eq!(compare_semver("0.1.22", "0.1.23"), -1);
    }

    #[test]
    fn test_variant_isolation_matching() {
        let releases = vec![GhRelease {
            tag_name: "v0.2.0".to_string(),
            draft: false,
            assets: vec![
                GhAsset {
                    name: "nfc-bridge-tools-0.2.0-windows-x64-portable.zip".to_string(),
                    browser_download_url: "https://example.com/tools.zip".to_string(),
                    size: 100,
                },
                GhAsset {
                    name: "nfc-bridge-sync-0.2.0-windows-x64-setup.exe".to_string(),
                    browser_download_url: "https://example.com/sync-setup.exe".to_string(),
                    size: 100,
                },
                GhAsset {
                    name: "nfc-bridge-sync-0.2.0-windows-x64-portable.zip".to_string(),
                    browser_download_url: "https://example.com/sync-portable.zip".to_string(),
                    size: 100,
                },
            ],
        }];

        let sync_portable = find_best_release_match(&releases, "nfc-bridge-sync-", true).unwrap();
        assert_eq!(
            sync_portable.asset_name,
            "nfc-bridge-sync-0.2.0-windows-x64-portable.zip"
        );
        assert_eq!(sync_portable.version, "0.2.0");

        let sync_setup = find_best_release_match(&releases, "nfc-bridge-sync-", false).unwrap();
        assert_eq!(
            sync_setup.asset_name,
            "nfc-bridge-sync-0.2.0-windows-x64-setup.exe"
        );

        let tools_setup = find_best_release_match(&releases, "nfc-bridge-tools-", false).unwrap();
        assert_eq!(
            tools_setup.asset_name,
            "nfc-bridge-tools-0.2.0-windows-x64-portable.zip"
        );
    }
}
