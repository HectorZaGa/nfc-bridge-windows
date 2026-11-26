include!("../win32_build_common.rs");

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=../../assets/icons/sync");
    println!("cargo:rerun-if-changed=ui/i18n");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=../win32_build_common.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
    println!("cargo:rerun-if-env-changed=NFC_BRIDGE_BUILD_ID");

    emit_build_with_icons(Win32ResourceSpec {
        prefix: "w",
        bin_name: "nfc-bridge-sync",
        title: "NFC Bridge Sync",
        desc: "NFC Bridge Sync — Escritorio PC/SC y puente móvil",
        orig_name: "nfc-bridge-sync.exe",
        app_ico_path: Some("../../assets/icons/sync/app.ico"),
        tray_ico_path: Some("../../assets/icons/sync/tray.ico"),
    });
}
