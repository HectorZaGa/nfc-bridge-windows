use crate::platform::hidden_command;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

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

pub(crate) fn default_updates_cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("NFCBridge")
        .join("updates")
}

pub(crate) fn apply_portable_archive_update(
    current_exe: &Path,
    archive_path: &Path,
) -> Result<PathBuf> {
    let exe_dir = current_exe
        .parent()
        .context("Sin carpeta para binario portable")?;
    let stage = exe_dir.join(".nb_update_tmp");
    let extracted_exe = extract_exe_from_zip(archive_path, &stage)?;

    let extracted_name = extracted_exe
        .file_name()
        .context("Nombre inválido en archivo extraído")?;
    let target_exe = exe_dir.join(extracted_name);

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

    if current_exe.eq(&target_exe)
        || current_exe.file_name().map(|n| n.to_ascii_lowercase())
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

pub(crate) fn extract_exe_from_zip(zip_path: &Path, stage_dir: &Path) -> Result<PathBuf> {
    if stage_dir.exists() {
        let _ = std::fs::remove_dir_all(stage_dir);
    }
    std::fs::create_dir_all(stage_dir)?;

    let mut extracted_ok = false;
    #[cfg(windows)]
    {
        if let Ok(st) = hidden_command("tar.exe")
            .args([
                "-xf",
                &zip_path.to_string_lossy(),
                "-C",
                &stage_dir.to_string_lossy(),
            ])
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
            if let Ok(st) = hidden_command("powershell.exe")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
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

pub(crate) fn launch_detached(exe: &Path, args: &[&str]) -> Result<()> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    cmd.spawn().context("No se pudo iniciar el ejecutable")?;
    Ok(())
}

pub(crate) fn stop_named_driver_processes() -> Result<()> {
    #[cfg(windows)]
    {
        let _ = hidden_command("taskkill")
            .args(["/IM", "nfc-bridge-driver.exe", "/F"])
            .status();
    }
    Ok(())
}
