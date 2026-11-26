include!("../win32_build_common.rs");
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=NFC_INSTALLER_PRODUCT");
    println!("cargo:rerun-if-env-changed=NFC_INSTALLER_VERSION");
    println!("cargo:rerun-if-env-changed=NFC_INSTALLER_PAYLOAD_PATH");
    println!("cargo:rerun-if-changed=../../assets/icons/sync");
    println!("cargo:rerun-if-changed=../../assets/icons/tools");
    println!("cargo:rerun-if-changed=../../../nfc-bridge-driver/assets/icons");
    println!("cargo:rerun-if-changed=../win32_build_common.rs");

    let product = std::env::var("NFC_INSTALLER_PRODUCT").unwrap_or_else(|_| "sync".to_string());
    let version = std::env::var("NFC_INSTALLER_VERSION")
        .unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string()));

    println!("cargo:rustc-env=NFC_INSTALLER_PRODUCT={}", product);
    println!("cargo:rustc-env=NFC_INSTALLER_VERSION={}", version);

    let (title, setup_ico, uninst_ico, bgra) = match product.as_str() {
        "tools" => (
            "NFC Bridge Tools Setup",
            "../../assets/icons/tools/setup.ico",
            "../../assets/icons/tools/uninstall.ico",
            [212, 182, 6, 255],
        ),
        "driver" => (
            "NFC Bridge Driver Setup",
            "../../../nfc-bridge-driver/assets/icons/setup.ico",
            "../../../nfc-bridge-driver/assets/icons/uninstall.ico",
            [129, 185, 16, 255],
        ),
        _ => (
            "NFC Bridge Sync Setup",
            "../../assets/icons/sync/setup.ico",
            "../../assets/icons/sync/uninstall.ico",
            [246, 130, 59, 255],
        ),
    };

    emit_build_with_icons(Win32ResourceSpec {
        prefix: "i",
        bin_name: "nfc-bridge-installer",
        title,
        desc: "NFC Bridge Windows Installer",
        orig_name: "nfc-bridge-installer.exe",
        app_ico_path: Some(setup_ico),
        tray_ico_path: Some(uninst_ico),
    });

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR missing"));
    let uninst_out = out_dir.join("uninstall.ico");
    let _ = std::fs::write(&uninst_out, load_ico_or_fallback(Some(uninst_ico), bgra));

    let dest = out_dir.join("payload.bin");
    if let Ok(src_raw) = std::env::var("NFC_INSTALLER_PAYLOAD_PATH") {
        if !src_raw.trim().is_empty() {
            let src = PathBuf::from(src_raw.trim());
            println!("cargo:rerun-if-changed={}", src.display());
            std::fs::copy(&src, &dest).expect("Failed copying installer payload binary");
            return;
        }
    }

    let _ = std::fs::write(&dest, b"");
}
