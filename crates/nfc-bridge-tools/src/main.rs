use anyhow::Result;
use core_proto::{FrameCodec, MessageType, NfcBridgeFrame};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let is_portable = cfg!(feature = "portable") || std::env::current_exe()
        .ok()
        .map(|p| {
            let in_dir = p.parent().map(|dir| dir.join("portable.dat").exists() || dir.join("nfc-bridge.json").exists()).unwrap_or(false);
            let in_name = p.file_name().and_then(|n| n.to_str()).map(|n| {
                let lower = n.to_lowercase();
                lower.contains("portable") || lower.ends_with("p.exe")
            }).unwrap_or(false);
            in_dir || in_name
        })
        .unwrap_or(false);

    println!("==================================================");
    println!("          NFC Bridge Tools - Windows Pro          ");
    if is_portable {
        println!("             [ MODO PORTABLE ACTIVO ]             ");
    }
    println!("==================================================");
    println!(" [1] Monitor de APDUs en tiempo real");
    println!(" [2] Comprobar lectores PC/SC del sistema");
    println!(" [3] Inyector / Emulador de respuestas APDU");
    println!(" [4] Diagnóstico de latencia USB y red local");
    println!("==================================================");

    info!("Iniciando suite de herramientas avanzadas...");
    let test_frame = NfcBridgeFrame::new(
        MessageType::ReaderStatus,
        1,
        bytes::Bytes::from_static(b"STATUS:ACTIVE"),
    );
    let encoded = FrameCodec::encode(&test_frame);
    info!("Trama de prueba BAP codificada ({} bytes)", encoded.len());

    Ok(())
}
