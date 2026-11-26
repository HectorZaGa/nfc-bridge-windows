#![windows_subsystem = "windows"]

//! Instalador y desinstalador nativo de Windows para NFC Bridge (Sync, Tools y Driver).
//!
//! Registra accesos directos, clave de desinstalación en Windows (Programas y características),
//! servicio del controlador virtual cuando corresponde y soporta desinstalación limpia (`--uninstall`).

use std::path::{Path, PathBuf};

static PAYLOAD_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));
static UNINSTALL_ICON_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/uninstall.ico"));
const PRODUCT_SLUG: &str = env!("NFC_INSTALLER_PRODUCT");
const PRODUCT_VERSION: &str = env!("NFC_INSTALLER_VERSION");

#[derive(Clone, Copy)]
struct ProductMeta {
    display_name: &'static str,
    folder_name: &'static str,
    exe_filename: &'static str,
    reg_key_name: &'static str,
    service_driver: bool,
    shortcut_args: &'static str,
}

fn product_meta() -> ProductMeta {
    match PRODUCT_SLUG {
        "tools" => ProductMeta {
            display_name: "NFC Bridge Tools",
            folder_name: "Bridge Tools",
            exe_filename: "NFC Bridge Tools.exe",
            reg_key_name: "NFCBridgeTools",
            service_driver: false,
            shortcut_args: "--open-ui",
        },
        "driver" => ProductMeta {
            display_name: "NFC Bridge Driver",
            folder_name: "Bridge Driver",
            exe_filename: "nfc-bridge-driver.exe",
            reg_key_name: "NFCBridgeDriver",
            service_driver: true,
            shortcut_args: "",
        },
        _ => ProductMeta {
            display_name: "NFC Bridge Sync",
            folder_name: "Bridge Sync",
            exe_filename: "NFC Bridge Sync.exe",
            reg_key_name: "NFCBridgeSync",
            service_driver: false,
            shortcut_args: "--open-ui",
        },
    }
}

fn show_msg(title: &str, text: &str, yes_no: bool, is_error: bool) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO,
        };
        let t_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let m_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let flags = if yes_no {
            MB_YESNO | MB_ICONQUESTION
        } else if is_error {
            MB_OK | MB_ICONERROR
        } else {
            MB_OK | MB_ICONINFORMATION
        };
        let res = unsafe { MessageBoxW(std::ptr::null_mut(), m_wide.as_ptr(), t_wide.as_ptr(), flags) };
        res == IDYES
    }
    #[cfg(not(windows))]
    {
        let _ = (title, text, yes_no, is_error);
        true
    }
}

fn run_hidden(program: &str, args: &[&str]) -> bool {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

fn install_dir(meta: &ProductMeta) -> PathBuf {
    let base = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    base.join("NFC Bridge").join(meta.folder_name)
}

fn stop_running_processes(meta: &ProductMeta) {
    if meta.service_driver {
        let _ = run_hidden("sc.exe", &["stop", "NfcBridgeDriver"]);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = run_hidden("taskkill", &["/IM", "nfc-bridge-driver.exe", "/F"]);
    } else {
        let _ = run_hidden("taskkill", &["/IM", meta.exe_filename, "/F"]);
        let _ = run_hidden("taskkill", &["/IM", "nfc-bridge-sync.exe", "/F"]);
        let _ = run_hidden("taskkill", &["/IM", "nfc-bridge-tools.exe", "/F"]);
    }
    std::thread::sleep(std::time::Duration::from_millis(250));
}

fn shortcut_paths(meta: &ProductMeta) -> Vec<PathBuf> {
    let mut list = Vec::new();
    if meta.service_driver {
        return list;
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        list.push(
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("NFC Bridge")
                .join(format!("{}.lnk", meta.display_name)),
        );
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        list.push(
            PathBuf::from(profile)
                .join("Desktop")
                .join(format!("{}.lnk", meta.display_name)),
        );
    }
    list
}

fn create_shortcut(lnk_path: &Path, target_exe: &Path, args: &str) {
    if let Some(parent) = lnk_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let lnk = lnk_path.to_string_lossy().replace('\'', "''");
    let tgt = target_exe.to_string_lossy().replace('\'', "''");
    let dir = target_exe
        .parent()
        .map(|p| p.to_string_lossy().replace('\'', "''"))
        .unwrap_or_default();
    let esc_args = args.replace('\'', "''");
    let ps = format!(
        "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.Arguments = '{}'; $s.WorkingDirectory = '{}'; $s.IconLocation = '{},0'; $s.Save()",
        lnk, tgt, esc_args, dir, tgt
    );
    let _ = run_hidden("powershell.exe", &["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps]);
}

fn register_uninstall_entry(meta: &ProductMeta, dir: &Path, uninstaller_exe: &Path, target_exe: &Path) {
    let hive_key = format!(r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\{}", meta.reg_key_name);
    let uninstall_ico = dir.join("uninstall.ico");
    let icon = if uninstall_ico.exists() {
        uninstall_ico.to_string_lossy().to_string()
    } else {
        format!("{},0", target_exe.display())
    };
    let uninst_cmd = format!("\"{}\" --uninstall", uninstaller_exe.display());
    let dir_str = dir.to_string_lossy().to_string();

    let entries = [
        ("DisplayName", "REG_SZ", meta.display_name),
        ("DisplayVersion", "REG_SZ", PRODUCT_VERSION),
        ("Publisher", "REG_SZ", "HectorZaGa"),
        ("InstallLocation", "REG_SZ", &dir_str),
        ("DisplayIcon", "REG_SZ", &icon),
        ("UninstallString", "REG_SZ", &uninst_cmd),
        ("QuietUninstallString", "REG_SZ", &format!("{} --silent", uninst_cmd)),
        ("NoModify", "REG_DWORD", "1"),
        ("NoRepair", "REG_DWORD", "1"),
    ];
    for (name, ty, val) in entries {
        let _ = run_hidden("reg", &["add", &hive_key, "/v", name, "/t", ty, "/d", val, "/f"]);
    }
}

fn unregister_uninstall_entry(meta: &ProductMeta) {
    let hive_key = format!(r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\{}", meta.reg_key_name);
    let _ = run_hidden("reg", &["delete", &hive_key, "/f"]);
}

fn run_install(meta: ProductMeta, silent: bool, auto_relaunch: bool) -> Result<(), String> {
    if PAYLOAD_BYTES.is_empty() {
        return Err("Este binario de instalador no contiene un paquete embebido.".to_string());
    }

    let dir = install_dir(&meta);
    if !silent {
        let prompt = format!(
            "¿Deseas instalar {} v{} en:\n{}\n\nSe incluirá acceso directo y desinstalador en Configuración de Windows.",
            meta.display_name,
            PRODUCT_VERSION,
            dir.display()
        );
        if !show_msg(&format!("Instalador de {}", meta.display_name), &prompt, true, false) {
            return Ok(());
        }
    }

    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear carpeta: {}", e))?;
    stop_running_processes(&meta);

    let target_exe = dir.join(meta.exe_filename);
    std::fs::write(&target_exe, PAYLOAD_BYTES)
        .map_err(|e| format!("No se pudo copiar ejecutable (ejecuta como administrador si es el Driver): {}", e))?;

    let uninstaller_exe = dir.join("uninstaller.exe");
    if let Ok(self_exe) = std::env::current_exe() {
        if self_exe != uninstaller_exe {
            let _ = std::fs::copy(&self_exe, &uninstaller_exe);
        }
    }
    if !UNINSTALL_ICON_BYTES.is_empty() {
        let _ = std::fs::write(dir.join("uninstall.ico"), UNINSTALL_ICON_BYTES);
    }

    if meta.service_driver {
        if !run_hidden(&target_exe.to_string_lossy(), &["--install-service"]) {
            return Err("No se pudo registrar/iniciar el servicio NfcBridgeDriver.".to_string());
        }
    } else {
        for lnk in shortcut_paths(&meta) {
            create_shortcut(&lnk, &target_exe, meta.shortcut_args);
        }
    }

    register_uninstall_entry(&meta, &dir, &uninstaller_exe, &target_exe);

    if auto_relaunch && !meta.service_driver {
        let mut cmd = std::process::Command::new(&target_exe);
        if !meta.shortcut_args.is_empty() {
            cmd.arg(meta.shortcut_args);
        }
        let _ = cmd.spawn();
    } else if !silent {
        let launch_now = if !meta.service_driver {
            show_msg(
                &format!("{} v{}", meta.display_name, PRODUCT_VERSION),
                "Instalación completada correctamente.\n\n¿Deseas abrir el programa ahora?",
                true,
                false,
            )
        } else {
            show_msg(
                &format!("{} v{}", meta.display_name, PRODUCT_VERSION),
                "Controlador instalado e iniciado correctamente.",
                false,
                false,
            );
            false
        };
        if launch_now {
            let mut cmd = std::process::Command::new(&target_exe);
            if !meta.shortcut_args.is_empty() {
                cmd.arg(meta.shortcut_args);
            }
            let _ = cmd.spawn();
        }
    }
    Ok(())
}

fn run_uninstall(meta: ProductMeta, silent: bool) -> Result<(), String> {
    if !silent {
        let prompt = format!(
            "¿Deseas desinstalar {} de este equipo?",
            meta.display_name
        );
        if !show_msg(&format!("Desinstalar {}", meta.display_name), &prompt, true, false) {
            return Ok(());
        }
    }

    let dir = install_dir(&meta);
    let target_exe = dir.join(meta.exe_filename);

    if meta.service_driver && target_exe.exists() {
        let _ = run_hidden(&target_exe.to_string_lossy(), &["--uninstall-service"]);
    }
    stop_running_processes(&meta);

    for lnk in shortcut_paths(&meta) {
        let _ = std::fs::remove_file(&lnk);
    }
    unregister_uninstall_entry(&meta);
    let _ = std::fs::remove_file(&target_exe);

    // Programar limpieza del directorio propio tras salir del proceso de desinstalación
    let dir_str = dir.to_string_lossy().replace('"', "");
    let cleanup_cmd = format!("ping -n 2 127.0.0.1 >nul & rd /s /q \"{}\"", dir_str);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd.exe")
            .args(["/C", &cleanup_cmd])
            .creation_flags(0x0800_0000)
            .spawn();
    }

    if !silent {
        show_msg(
            &format!("Desinstalador de {}", meta.display_name),
            "Se ha desinstalado correctamente de Windows.",
            false,
            false,
        );
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let uninstall = args.iter().any(|a| a.eq_ignore_ascii_case("--uninstall") || a.eq_ignore_ascii_case("/uninstall"));
    let is_update = args.iter().any(|a| {
        a.eq_ignore_ascii_case("/UPDATE")
            || a.eq_ignore_ascii_case("--update")
            || a.eq_ignore_ascii_case("/VERYSILENT")
    });
    let silent = is_update
        || args.iter().any(|a| {
            a.eq_ignore_ascii_case("/S")
                || a.eq_ignore_ascii_case("/SILENT")
                || a.eq_ignore_ascii_case("--silent")
                || a.eq_ignore_ascii_case("/SUPPRESSMSGBOXES")
        });

    let meta = product_meta();
    let res = if uninstall {
        run_uninstall(meta, silent)
    } else {
        run_install(meta, silent, is_update)
    };

    if let Err(err) = res {
        if !silent {
            show_msg(&format!("Error - {}", meta.display_name), &err, false, true);
        }
        std::process::exit(1);
    }
}
