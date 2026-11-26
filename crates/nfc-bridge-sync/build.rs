use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
    println!("cargo:rerun-if-env-changed=NFC_BRIDGE_BUILD_ID");

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
        .unwrap_or_else(|| format!("w{}-{}-{}", git_rev, git_sha, ts));

    println!("cargo:rustc-env=NFC_BRIDGE_GIT_SHA={}", git_sha);
    println!("cargo:rustc-env=NFC_BRIDGE_BUILD_ID={}", build_id);

    copy_webview2_loader_for_gnu();
}

fn copy_webview2_loader_for_gnu() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x64",
        Ok("aarch64") => "arm64",
        Ok("x86") => "x86",
        _ => return,
    };
    let Ok(out_dir) = std::env::var("OUT_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    // OUT_DIR = target/<profile>/build/nfc-bridge-sync-<hash>/out
    let Some(profile_dir) = out_dir.ancestors().nth(3) else {
        return;
    };
    let build_dir = profile_dir.join("build");
    let Ok(entries) = std::fs::read_dir(&build_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("webview2-com-sys-") {
            continue;
        }
        let candidate = entry.path().join("out").join(arch).join("WebView2Loader.dll");
        if candidate.is_file() {
            let _ = std::fs::copy(&candidate, profile_dir.join("WebView2Loader.dll"));
            break;
        }
    }
}
