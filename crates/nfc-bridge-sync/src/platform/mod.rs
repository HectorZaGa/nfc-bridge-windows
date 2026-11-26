#[cfg(windows)]
mod win32;

#[cfg(windows)]
pub use win32::{
    apply_win32_dwm_to_title, detect_windows_dark_mode, detect_windows_spanish_locale,
    launch_elevated_or_normal, read_windows_accent_hex, replace_previous_sync_processes,
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
