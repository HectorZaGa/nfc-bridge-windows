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

    println!("==================================================");
    println!("          NFC Bridge Tools - Windows Pro          ");
    println!("==================================================");
    println!(" [1] Monitor de APDUs en tiempo real");
    println!(" [2] Comprobar lectores PC/SC del sistema");
    println!(" [3] Inyector / Emulador de respuestas APDU");
    println!(" [4] Diagnóstico de latencia USB y red local");
    println!("==================================================");

    info!("Iniciando suite de herramientas avanzadas...");
    // Demo de frame de diagnóstico
    let test_frame = NfcBridgeFrame::new(
        MessageType::ReaderStatus,
        1,
        bytes::Bytes::from_static(b"STATUS:ACTIVE"),
    );
    let encoded = FrameCodec::encode(&test_frame);
    info!("Trama de prueba BAP codificada ({} bytes)", encoded.len());

    Ok(())
}
