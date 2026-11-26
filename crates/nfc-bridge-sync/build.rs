include!("../win32_build_common.rs");

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=ui/i18n");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=../win32_build_common.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
    println!("cargo:rerun-if-env-changed=NFC_BRIDGE_BUILD_ID");

    emit_build_env_and_resources(
        "w",
        "nfc-bridge-sync",
        "NFC Bridge Sync",
        "NFC Bridge Sync — Escritorio PC/SC y puente móvil",
        "nfc-bridge-sync.exe",
    );
}
