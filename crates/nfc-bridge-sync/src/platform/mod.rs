#[cfg(windows)]
mod win32;

#[cfg(windows)]
pub use win32::{
    apply_win32_dwm_to_title, configure_tray_popup_win32, current_tray_input_mask_win32,
    detect_windows_dark_mode, detect_windows_spanish_locale, hide_tray_popup_win32,
    is_cursor_in_window_win32, launch_elevated_or_normal, poll_tray_input_edges_win32,
    prepare_cloaked_window_win32, read_windows_accent_hex, replace_previous_sync_processes,
    restore_main_window_win32, show_tray_popup_at_win32, uncloak_window_win32,
};

/// Crea un `Command` ocultando ventanas de consola auxiliares en Windows (`CREATE_NO_WINDOW`).
pub fn hidden_command<S: AsRef<std::ffi::OsStr>>(program: S) -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(program);
        cmd.creation_flags(0x0800_0000);
        cmd
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(program)
    }
}

#[cfg(not(windows))]
pub fn detect_windows_dark_mode() -> bool {
    true
}

#[cfg(not(windows))]
pub fn detect_windows_spanish_locale() -> bool {
    false
}

#[cfg(not(windows))]
pub fn read_windows_accent_hex() -> String {
    "#3B82F6".to_string()
}

#[cfg(not(windows))]
pub fn replace_previous_sync_processes() -> usize {
    0
}

#[cfg(not(windows))]
pub fn launch_elevated_or_normal(
    exe: &std::path::Path,
    args: &[&str],
) -> anyhow::Result<()> {
    use anyhow::Context;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    cmd.spawn().context("No se pudo iniciar el ejecutable")?;
    Ok(())
}
