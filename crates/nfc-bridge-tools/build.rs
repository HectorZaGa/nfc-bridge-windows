include!("../win32_build_common.rs");

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=../../assets/icons/tools");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=../win32_build_common.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
    println!("cargo:rerun-if-env-changed=NFC_BRIDGE_BUILD_ID");

    emit_build_with_icons(Win32ResourceSpec {
        prefix: "t",
        bin_name: "nfc-bridge-tools",
        title: "NFC Bridge Tools",
        desc: "NFC Bridge Tools — Diagnóstico y pruebas NFC de escritorio",
        orig_name: "nfc-bridge-tools.exe",
        app_ico_path: Some("../../assets/icons/tools/app.ico"),
        tray_ico_path: Some("../../assets/icons/tools/tray.ico"),
        uninstall_ico_path: Some("../../assets/icons/tools/uninstall.ico"),
    });
}
