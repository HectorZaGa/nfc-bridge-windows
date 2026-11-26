use anyhow::{Context, Result};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, POINT, RECT};
use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CLOAK, DWMWA_TRANSITIONS_FORCEDISABLED,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, GetMonitorInfoW, MonitorFromPoint, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetCursorPos, GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindowVisible,
    SetClassLongPtrW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    GCLP_HBRBACKGROUND, GWL_EXSTYLE, HWND_TOP, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
    SWP_SHOWWINDOW, SW_HIDE, SW_RESTORE, SW_SHOW, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
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

fn find_hwnd_by_title(title: &str) -> windows_sys::Win32::Foundation::HWND {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) }
}

unsafe fn set_dwm_bool_attr(hwnd: windows_sys::Win32::Foundation::HWND, attr: u32, val: bool) {
    let flag: i32 = if val { 1 } else { 0 };
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attr,
            &flag as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        )
    };
}

/// Configura el popup del tray con `WS_EX_NOACTIVATE` y `WS_EX_TOOLWINDOW` para que nunca
/// robe el foco de la bandeja de Windows (`^`) al abrirse o recibir clics.
pub fn configure_tray_popup_win32(title: &str) {
    let hwnd = find_hwnd_by_title(title);
    if hwnd.is_null() {
        return;
    }
    unsafe {
        set_dwm_bool_attr(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED as u32, true);
        if IsWindowVisible(hwnd) == 0 {
            set_dwm_bool_attr(hwnd, DWMWA_CLOAK as u32, true);
        }
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let desired = (ex & !(WS_EX_APPWINDOW as isize))
            | (WS_EX_NOACTIVATE as isize)
            | (WS_EX_TOOLWINDOW as isize)
            | (WS_EX_TOPMOST as isize);
        if desired != ex {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired);
        }
    }
}

/// Prepara y muestra un viewport oculto tras `DWMWA_CLOAK` en su posición final para que
/// pinte su primer frame con `swap_buffers` antes de revelarse sin destello negro ni salto visual.
pub fn prepare_cloaked_window_win32(
    title: &str,
    x_logical: f32,
    y_logical: f32,
    w_logical: f32,
    h_logical: f32,
    ppp: f32,
) {
    let hwnd = find_hwnd_by_title(title);
    if hwnd.is_null() {
        return;
    }
    let ppp = ppp.max(1.0);
    let x_px = (x_logical * ppp).round() as i32;
    let y_px = (y_logical * ppp).round() as i32;
    let w_px = (w_logical * ppp).round().max(100.0) as i32;
    let h_px = (h_logical * ppp).round().max(100.0) as i32;
    unsafe {
        set_dwm_bool_attr(hwnd, DWMWA_TRANSITIONS_FORCEDISABLED as u32, true);
        set_dwm_bool_attr(hwnd, DWMWA_CLOAK as u32, true);
        SetWindowPos(
            hwnd,
            HWND_TOP,
            x_px,
            y_px,
            w_px,
            h_px,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER,
        );
    }
}

/// Quita `DWMWA_CLOAK` una vez completado el primer `swap_buffers` y opcionalmente enfoca la ventana.
pub fn uncloak_window_win32(title: &str, focus: bool) {
    let hwnd = find_hwnd_by_title(title);
    if hwnd.is_null() {
        return;
    }
    unsafe {
        set_dwm_bool_attr(hwnd, DWMWA_CLOAK as u32, false);
        if focus {
            SetForegroundWindow(hwnd);
        }
    }
}

/// Muestra y posiciona el popup del tray sin activar ventana (`SWP_NOACTIVATE`).
/// Cuando `cloak_before_show` es `true`, permanece invisible mediante `DWMWA_CLOAK` hasta el
/// primer `swap_buffers` para evitar cualquier destello de 1 ms al abrir.
pub fn show_tray_popup_at_win32(
    title: &str,
    click_x_px: i32,
    click_y_px: i32,
    w_logical: f32,
    h_logical: f32,
    ppp: f32,
    cloak_before_show: bool,
) -> Option<(f32, f32)> {
    let ppp = ppp.max(1.0);
    let w_px = (w_logical * ppp).round().max(100.0) as i32;
    let h_px = (h_logical * ppp).round().max(100.0) as i32;

    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let pt = POINT {
        x: click_x_px,
        y: click_y_px,
    };
    let hmon = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    let (wl, wt, wr, wb) = if !hmon.is_null() && unsafe { GetMonitorInfoW(hmon, &mut mi) } != 0 {
        (
            mi.rcWork.left,
            mi.rcWork.top,
            mi.rcWork.right,
            mi.rcWork.bottom,
        )
    } else {
        (0, 0, 1920, 1040)
    };

    let pad = (8.0 * ppp).round() as i32;
    let gap = (10.0 * ppp).round() as i32;
    let x_px = (click_x_px - w_px + (18.0 * ppp).round() as i32)
        .clamp(wl + pad, (wr - w_px - pad).max(wl + pad));
    let mid_y = wt + (wb - wt) / 2;
    let raw_y = if click_y_px >= mid_y {
        click_y_px - h_px - gap
    } else {
        click_y_px + gap
    };
    let y_px = raw_y.clamp(wt + pad, (wb - h_px - pad).max(wt + pad));

    let hwnd = find_hwnd_by_title(title);
    if !hwnd.is_null() {
        configure_tray_popup_win32(title);
        unsafe {
            if cloak_before_show {
                set_dwm_bool_attr(hwnd, DWMWA_CLOAK as u32, true);
            }
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x_px,
                y_px,
                w_px,
                h_px,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER,
            );
        }
    }

    Some((x_px as f32 / ppp, y_px as f32 / ppp))
}

/// Posiciona y muestra el submenú en cascada junto a la fila padre del tray (`SWP_NOACTIVATE`)
/// a la derecha si hay espacio en el monitor o a la izquierda si está junto al borde derecho.
#[allow(clippy::too_many_arguments)]
pub fn show_tray_submenu_at_win32(
    sub_title: &str,
    parent_tray_title: &str,
    row_top_logical: f32,
    sub_w_logical: f32,
    sub_h_logical: f32,
    ppp: f32,
    cloak_before_show: bool,
) -> Option<(f32, f32)> {
    let parent_hwnd = find_hwnd_by_title(parent_tray_title);
    if parent_hwnd.is_null() {
        return None;
    }
    let mut p_rect: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(parent_hwnd, &mut p_rect) } == 0 {
        return None;
    }

    let ppp = ppp.max(1.0);
    let w_px = (sub_w_logical * ppp).round().max(120.0) as i32;
    let h_px = (sub_h_logical * ppp).round().max(60.0) as i32;

    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let pt = POINT {
        x: (p_rect.left + p_rect.right) / 2,
        y: (p_rect.top + p_rect.bottom) / 2,
    };
    let hmon = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    let (wl, wt, wr, wb) = if !hmon.is_null() && unsafe { GetMonitorInfoW(hmon, &mut mi) } != 0 {
        (
            mi.rcWork.left,
            mi.rcWork.top,
            mi.rcWork.right,
            mi.rcWork.bottom,
        )
    } else {
        (0, 0, 1920, 1040)
    };

    let pad = (8.0 * ppp).round() as i32;
    let gap = (4.0 * ppp).round() as i32;
    let x_px = if p_rect.right + gap + w_px <= wr - pad {
        p_rect.right + gap
    } else {
        (p_rect.left - gap - w_px).max(wl + pad)
    };
    let raw_y = p_rect.top + ((row_top_logical - 6.0) * ppp).round() as i32;
    let y_px = raw_y.clamp(wt + pad, (wb - h_px - pad).max(wt + pad));

    let sub_hwnd = find_hwnd_by_title(sub_title);
    if !sub_hwnd.is_null() {
        configure_tray_popup_win32(sub_title);
        unsafe {
            if cloak_before_show {
                set_dwm_bool_attr(sub_hwnd, DWMWA_CLOAK as u32, true);
            }
            SetWindowPos(
                sub_hwnd,
                HWND_TOPMOST,
                x_px,
                y_px,
                w_px,
                h_px,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER,
            );
        }
    }

    Some((x_px as f32 / ppp, y_px as f32 / ppp))
}

pub fn hide_tray_popup_win32(title: &str) {
    let hwnd = find_hwnd_by_title(title);
    if !hwnd.is_null() {
        unsafe {
            set_dwm_bool_attr(hwnd, DWMWA_CLOAK as u32, true);
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

pub fn restore_main_window_win32(title: &str) {
    let hwnd = find_hwnd_by_title(title);
    if hwnd.is_null() {
        return;
    }
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }
        SetForegroundWindow(hwnd);
    }
}

pub fn is_cursor_in_window_win32(title: &str) -> bool {
    let hwnd = find_hwnd_by_title(title);
    if hwnd.is_null() {
        return false;
    }
    unsafe {
        if IsWindowVisible(hwnd) == 0 {
            return false;
        }
        let mut pt: POINT = std::mem::zeroed();
        let mut rect: RECT = std::mem::zeroed();
        if GetCursorPos(&mut pt) == 0 || GetWindowRect(hwnd, &mut rect) == 0 {
            return false;
        }
        pt.x >= rect.left && pt.x <= rect.right && pt.y >= rect.top && pt.y <= rect.bottom
    }
}

pub fn current_tray_input_mask_win32() -> u8 {
    unsafe {
        let mut mask = 0u8;
        if (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 {
            mask |= 0b0001;
        }
        if (GetAsyncKeyState(VK_RBUTTON as i32) as u16 & 0x8000) != 0 {
            mask |= 0b0010;
        }
        if (GetAsyncKeyState(VK_MBUTTON as i32) as u16 & 0x8000) != 0 {
            mask |= 0b0100;
        }
        if (GetAsyncKeyState(VK_ESCAPE as i32) as u16 & 0x8000) != 0 {
            mask |= 0b1000;
        }
        mask
    }
}

pub fn poll_tray_input_edges_win32(prev_mask: &mut u8) -> (bool, bool) {
    let cur = current_tray_input_mask_win32();
    let rising = cur & !*prev_mask;
    *prev_mask = cur;
    let mouse_down = (rising & 0b0111) != 0;
    let esc_down = (rising & 0b1000) != 0;
    (mouse_down, esc_down)
}

const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn is_windows_autostart_enabled(value_name: &str) -> bool {
    let subkey_w: Vec<u16> = RUN_SUBKEY.encode_utf16().chain(std::iter::once(0)).collect();
    let value_w: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut cb_data: u32 = 0;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey_w.as_ptr(),
            value_w.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut cb_data,
        )
    };
    status == 0 && cb_data > 2
}

pub fn set_windows_autostart(value_name: &str, enabled: bool) -> bool {
    let subkey_w: Vec<u16> = RUN_SUBKEY.encode_utf16().chain(std::iter::once(0)).collect();
    let value_w: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: HKEY = std::ptr::null_mut();
    let open_rc = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey_w.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
    };
    if open_rc != 0 || hkey.is_null() {
        return false;
    }

    let ok = if enabled {
        if let Ok(exe_path) = std::env::current_exe() {
            let cmd = format!("\"{}\" --minimized", exe_path.display());
            let cmd_w: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes_len = (cmd_w.len() * std::mem::size_of::<u16>()) as u32;
            unsafe {
                RegSetValueExW(
                    hkey,
                    value_w.as_ptr(),
                    0,
                    REG_SZ,
                    cmd_w.as_ptr() as *const u8,
                    bytes_len,
                ) == 0
            }
        } else {
            false
        }
    } else {
        let rc = unsafe { RegDeleteValueW(hkey, value_w.as_ptr()) };
        rc == 0 || rc == 2 // ERROR_FILE_NOT_FOUND
    };

    unsafe {
        let _ = RegCloseKey(hkey);
    }
    ok
}
