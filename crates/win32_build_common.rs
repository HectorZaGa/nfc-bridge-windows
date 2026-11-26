use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Win32ResourceSpec<'a> {
    pub prefix: &'a str,
    pub bin_name: &'a str,
    pub title: &'a str,
    pub desc: &'a str,
    pub orig_name: &'a str,
    pub app_ico_path: Option<&'a str>,
    pub tray_ico_path: Option<&'a str>,
    pub uninstall_ico_path: Option<&'a str>,
}

pub fn emit_build_env_and_resources(
    prefix: &str,
    bin_name: &str,
    title: &str,
    desc: &str,
    orig_name: &str,
) {
    emit_build_with_icons(Win32ResourceSpec {
        prefix,
        bin_name,
        title,
        desc,
        orig_name,
        app_ico_path: None,
        tray_ico_path: None,
        uninstall_ico_path: None,
    });
}

pub fn emit_build_with_icons(spec: Win32ResourceSpec<'_>) {
    let git_sha = git_out(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "nogit".to_string());
    let git_rev = git_out(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "0".to_string());
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ts = utc_compact_from_unix(secs);

    let build_id = std::env::var("NFC_BRIDGE_BUILD_ID")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}{}-{}-{}", spec.prefix, git_rev, git_sha, ts));

    println!("cargo:rustc-env=NFC_BRIDGE_GIT_SHA={}", git_sha);
    println!("cargo:rustc-env=NFC_BRIDGE_BUILD_ID={}", build_id);

    emit_win32_exe_resources(&spec);
}

fn utc_compact_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3_600;
    let min = (rem % 3_600) / 60;
    let sec = rem % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    format!(
        "{:02}{:02}{:02}{:02}{:02}{:02}",
        (year % 100).abs(),
        m,
        d,
        hour,
        min,
        sec
    )
}

fn git_out(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

pub fn load_ico_or_fallback(custom_path: Option<&str>, accent_bgra: [u8; 4]) -> Vec<u8> {
    if let Some(p) = custom_path {
        let path = Path::new(p);
        if path.exists() {
            if let Ok(bytes) = std::fs::read(path) {
                if bytes.len() >= 22 {
                    return bytes;
                }
            }
        }
    }
    build_ico_bytes(accent_bgra)
}

fn emit_win32_exe_resources(spec: &Win32ResourceSpec<'_>) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = match std::env::var_os("OUT_DIR") {
        Some(p) => std::path::PathBuf::from(p),
        None => return,
    };
    let ver = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let parts: Vec<u16> = ver
        .split('.')
        .take(3)
        .map(|s| s.parse::<u16>().unwrap_or(0))
        .collect();
    let (v0, v1, v2) = (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    );

    let app_bgra = if spec.bin_name.contains("tools") {
        [212, 182, 6, 255] // #06B6D4 cyan para diferenciar Tools
    } else if spec.bin_name.contains("driver") {
        [129, 185, 16, 255] // #10B981 emerald para Driver
    } else {
        [246, 130, 59, 255] // #3B82F6 blue para Sync
    };

    let ico_path = out_dir.join("app.ico");
    let tray_ico_path = out_dir.join("tray.ico");
    let uninst_ico_path = out_dir.join("uninstall_res.ico");
    let rc_path = out_dir.join("app.rc");
    let res_path = out_dir.join("app.res");

    if std::fs::write(&ico_path, load_ico_or_fallback(spec.app_ico_path, app_bgra)).is_err() {
        return;
    }

    let mut tray_rc_line = String::new();
    let mut next_icon_id = 2u8;
    if let Some(tp) = spec.tray_ico_path {
        let tray_bytes = load_ico_or_fallback(Some(tp), app_bgra);
        if std::fs::write(&tray_ico_path, tray_bytes).is_ok() {
            tray_rc_line = format!("{} ICON \"tray.ico\"\n", next_icon_id);
            next_icon_id += 1;
        }
    }
    let mut uninst_rc_line = String::new();
    if let Some(up) = spec.uninstall_ico_path {
        let up_path = Path::new(up);
        if up_path.exists() {
            if let Ok(uninst_bytes) = std::fs::read(up_path) {
                if uninst_bytes.len() >= 22 && std::fs::write(&uninst_ico_path, uninst_bytes).is_ok()
                {
                    uninst_rc_line = format!("{} ICON \"uninstall_res.ico\"\n", next_icon_id);
                }
            }
        }
    }

    let bin_name = spec.bin_name;
    let title = spec.title;
    let desc = spec.desc;
    let orig_name = spec.orig_name;

    let mut manifest_rc_line = String::new();
    if bin_name == "nfc-bridge-installer" && title.contains("Driver") {
        let manifest_path = out_dir.join("app.manifest");
        let manifest_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;
        if std::fs::write(&manifest_path, manifest_xml).is_ok() {
            manifest_rc_line = "1 24 \"app.manifest\"\n".to_string();
        }
    }

    let rc_content = format!(
        r#"1 ICON "app.ico"
{tray_rc_line}{uninst_rc_line}{manifest_rc_line}1 VERSIONINFO
FILEVERSION {v0},{v1},{v2},0
PRODUCTVERSION {v0},{v1},{v2},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "NFC Bridge\0"
      VALUE "FileDescription", "{title}\0"
      VALUE "Comments", "{desc}\0"
      VALUE "FileVersion", "{ver}\0"
      VALUE "InternalName", "{bin_name}\0"
      VALUE "OriginalFilename", "{orig_name}\0"
      VALUE "ProductName", "{title}\0"
      VALUE "ProductVersion", "{ver}\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0409, 1200
  END
END
"#
    );
    if std::fs::write(&rc_path, rc_content).is_err() {
        return;
    }

    let res_format = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        "res"
    } else {
        "coff"
    };
    let ok = Command::new("windres")
        .current_dir(&out_dir)
        .args(["app.rc", "-O", res_format, "-o", "app.res"])
        .status()
        .is_ok_and(|s| s.success());
    if ok && res_path.exists() {
        println!("cargo:rustc-link-arg-bin={}={}", bin_name, res_path.display());
    }
}

pub fn build_ico_bytes(accent_bgra: [u8; 4]) -> Vec<u8> {
    let size = 32usize;
    let dib_size = 40 + size * size * 4 + size * 4;
    let mut buf = Vec::with_capacity(6 + 16 + dib_size);
    buf.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    buf.push(32);
    buf.push(32);
    buf.push(0);
    buf.push(0);
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&32u16.to_le_bytes());
    buf.extend_from_slice(&(dib_size as u32).to_le_bytes());
    buf.extend_from_slice(&22u32.to_le_bytes());
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&(size as i32).to_le_bytes());
    buf.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&32u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&((size * size * 4) as u32).to_le_bytes());
    buf.extend_from_slice(&[0u8; 16]);

    for y in (0..size).rev() {
        for x in 0..size {
            let dx = (x as f32 - 15.5).abs();
            let dy = (y as f32 - 15.5).abs();
            let inside = if dx > 10.5 && dy > 10.5 {
                ((dx - 10.5).powi(2) + (dy - 10.5).powi(2)).sqrt() <= 5.0
            } else {
                dx <= 15.5 && dy <= 15.5
            };
            if !inside {
                buf.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let cx = x as f32 - 11.5;
                let cy = y as f32 - 15.5;
                let dist = (cx * cx + cy * cy).sqrt();
                let in_arc = cx >= -1.0
                    && ((3.2..=5.4).contains(&dist)
                        || (7.6..=9.8).contains(&dist)
                        || (12.0..=14.2).contains(&dist));
                let in_dot =
                    ((x as f32 - 10.5).powi(2) + (y as f32 - 15.5).powi(2)).sqrt() <= 2.1;
                if in_arc || in_dot {
                    buf.extend_from_slice(&[255, 255, 255, 255]);
                } else {
                    buf.extend_from_slice(&accent_bgra);
                }
            }
        }
    }
    buf.resize(buf.len() + size * 4, 0);
    buf
}
