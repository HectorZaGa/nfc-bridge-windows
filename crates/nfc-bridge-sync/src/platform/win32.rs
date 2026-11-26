use anyhow::{Context, Result};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows_sys::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows_sys::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetClassLongPtrW, GCLP_HBRBACKGROUND,
};

fn read_hkcu_dword(subkey: &str, value_name: &str) -> Option<u32> {
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let value_w: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut data: u32 = 0;
    let mut cb_data: u32 = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey_w.as_ptr(),
            value_w.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut core::ffi::c_void,
            &mut cb_data,
        )
    };
    if status == 0 {
        Some(data)
    } else {
        None
    }
}

/// Detecta modo oscuro de Windows con caché de 2s usando `RegGetValueW` (sin subprocesos).
pub fn detect_windows_dark_mode() -> bool {
    static CACHE: OnceLock<Mutex<Option<(Instant, bool)>>> = OnceLock::new();
    let lock = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        let now = Instant::now();
        if let Some((ts, cached)) = *guard {
            if now.duration_since(ts) < Duration::from_secs(2) {
                return cached;
            }
        }
        let dark = read_hkcu_dword(
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "AppsUseLightTheme",
        )
        .is_none_or(|v| v == 0);
        *guard = Some((now, dark));
        return dark;
    }
    true
}

/// Lee el color de acento DWM de Windows en formato `#RRGGBB` con caché de 2s vía `RegGetValueW`.
pub fn read_windows_accent_hex() -> String {
    static CACHE: OnceLock<Mutex<Option<(Instant, String)>>> = OnceLock::new();
    let lock = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        let now = Instant::now();
        if let Some((ts, ref cached)) = *guard {
            if now.duration_since(ts) < Duration::from_secs(2) {
                return cached.clone();
            }
        }
        let hex = read_hkcu_dword(r"Software\Microsoft\Windows\DWM", "AccentColor")
            .map(|abgr| {
                let r = (abgr & 0xFF) as u8;
                let g = ((abgr >> 8) & 0xFF) as u8;
                let b = ((abgr >> 16) & 0xFF) as u8;
                format!("#{:02X}{:02X}{:02X}", r, g, b)
            })
            .unwrap_or_else(|| "#3B82F6".to_string());
        *guard = Some((now, hex.clone()));
        return hex;
    }
    "#3B82F6".to_string()
}

pub fn detect_windows_spanish_locale() -> bool {
    let mut buf = [0u16; 85];
    let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if len > 1 {
        let s = String::from_utf16_lossy(&buf[..(len as usize - 1)]).to_ascii_lowercase();
        return s.starts_with("es");
    }
    false
}

/// Aplica esquinas redondeadas DWM de Windows 11, color de borde nativo y pincel de fondo al HWND.
pub fn apply_win32_dwm_to_title(
    title: &str,
    surface_rgb: (u8, u8, u8),
    outline_rgb: (u8, u8, u8),
    is_dark: bool,
) {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let corner_pref: u32 = DWMWCP_ROUND as u32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &corner_pref as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let border_color: u32 = ((outline_rgb.2 as u32) << 16)
            | ((outline_rgb.1 as u32) << 8)
            | (outline_rgb.0 as u32);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &border_color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let dark_flag: i32 = if is_dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark_flag as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );
        let bg_color: u32 = ((surface_rgb.2 as u32) << 16)
            | ((surface_rgb.1 as u32) << 8)
            | (surface_rgb.0 as u32);
        let brush = CreateSolidBrush(bg_color);
        if !brush.is_null() {
            let prev = SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, brush as isize);
            if prev != 0 {
                let _ = DeleteObject(prev as _);
            }
        }
    }
}

/// Cierra instancias previas de `nfc-bridge-sync` antes de inicializar el icono de bandeja y el IPC.
pub fn replace_previous_sync_processes() -> usize {
    let self_pid = std::process::id();
    let mut killed = 0usize;

    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return 0;
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let pid = entry.th32ProcessID;
                if pid != 0 && pid != self_pid {
                    let end = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();

                    if exe_name.starts_with("nfc-bridge-sync")
                        || exe_name.starts_with("nfc bridge sync")
                    {
                        let proc_handle =
                            OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, pid);
                        if !proc_handle.is_null() {
                            if TerminateProcess(proc_handle, 0) != 0 {
                                let _ = WaitForSingleObject(proc_handle, 1500);
                                killed += 1;
                            }
                            let _ = CloseHandle(proc_handle);
                        }
                    }
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }

        let _ = CloseHandle(snap);
    }
    killed
}

pub fn launch_elevated_or_normal(exe: &Path, args: &[&str]) -> Result<()> {
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
    let status = super::hidden_command("powershell.exe")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .status()
        .context("No se pudo elevar permisos de instalación")?;
    if !status.success() {
        warn!("Elevación UAC cancelada o fallida para {:?}", exe);
        anyhow::bail!("Instalación cancelada o sin permisos elevados");
    }
    Ok(())
}
