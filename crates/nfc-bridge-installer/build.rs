use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=NFC_INSTALLER_PRODUCT");
    println!("cargo:rerun-if-env-changed=NFC_INSTALLER_VERSION");
    println!("cargo:rerun-if-env-changed=NFC_INSTALLER_PAYLOAD_PATH");
    println!("cargo:rerun-if-env-changed=NFC_INSTALLER_WEBVIEW2_DLL");

    let product = std::env::var("NFC_INSTALLER_PRODUCT").unwrap_or_else(|_| "sync".to_string());
    let version = std::env::var("NFC_INSTALLER_VERSION")
        .unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string()));

    println!("cargo:rustc-env=NFC_INSTALLER_PRODUCT={}", product);
    println!("cargo:rustc-env=NFC_INSTALLER_VERSION={}", version);

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR missing"));
    let dest = out_dir.join("payload.bin");
    let wv2_dest = out_dir.join("webview2loader.bin");

    let mut wrote_wv2 = false;
    if let Ok(wv2_raw) = std::env::var("NFC_INSTALLER_WEBVIEW2_DLL") {
        if !wv2_raw.trim().is_empty() {
            let src = PathBuf::from(wv2_raw.trim());
            println!("cargo:rerun-if-changed={}", src.display());
            std::fs::copy(&src, &wv2_dest).expect("Failed copying WebView2Loader.dll for installer");
            wrote_wv2 = true;
        }
    }
    if !wrote_wv2 {
        let _ = std::fs::write(&wv2_dest, b"");
    }

    if let Ok(src_raw) = std::env::var("NFC_INSTALLER_PAYLOAD_PATH") {
        if !src_raw.trim().is_empty() {
            let src = PathBuf::from(src_raw.trim());
            println!("cargo:rerun-if-changed={}", src.display());
            std::fs::copy(&src, &dest).expect("Failed copying installer payload binary");
            return;
        }
    }

    // Fallback vacío para `cargo check` / `cargo test` sin empaquetado release
    let _ = std::fs::write(&dest, b"");
}
