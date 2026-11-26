/// Construye el URI estándar `nfcbridge://` con parámetros de Wi-Fi Direct (SoftAP + TCP) para QR.
pub fn build_wifi_direct_qr_uri(
    ip: &str,
    port: u16,
    pc_name: &str,
    ssid: &str,
    passphrase: &str,
) -> String {
    format!(
        "nfcbridge://{}:{}?name={}&transport=wifi_direct&ssid={}&pass={}",
        ip,
        port,
        percent_encode_query(pc_name),
        percent_encode_query(ssid),
        percent_encode_query(passphrase),
    )
}

pub fn percent_encode_query(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

pub fn normalize_hex_color(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches('#');
    if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{}", trimmed.to_ascii_uppercase())
    } else {
        "#3B82F6".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wifi_direct_qr_uri_encoding() {
        let uri = build_wifi_direct_qr_uri(
            "192.168.137.1",
            45362,
            "HECTOR MASTER",
            "DIRECT-NB-HECTOR",
            "Abc123XY",
        );
        assert_eq!(
            uri,
            "nfcbridge://192.168.137.1:45362?name=HECTOR%20MASTER&transport=wifi_direct&ssid=DIRECT-NB-HECTOR&pass=Abc123XY"
        );
    }
}
