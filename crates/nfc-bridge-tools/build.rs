include!("../win32_build_common.rs");

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=../win32_build_common.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
    println!("cargo:rerun-if-env-changed=NFC_BRIDGE_BUILD_ID");

    emit_build_env_and_resources(
        "t",
        "nfc-bridge-tools",
        "NFC Bridge Tools",
        "NFC Bridge Tools — Diagnóstico y pruebas NFC de escritorio",
        "nfc-bridge-tools.exe",
    );
}
