pub(crate) fn load_app_icon_data(ico_bytes: &[u8]) -> eframe::egui::IconData {
    if let Some(icon) = decode_ico_to_egui_icon(ico_bytes) {
        return icon;
    }
    eframe::egui::IconData {
        rgba: build_nfc_rgba_bytes(),
        width: 32,
        height: 32,
    }
}

fn decode_ico_to_egui_icon(ico_bytes: &[u8]) -> Option<eframe::egui::IconData> {
    if ico_bytes.len() < 6 {
        return None;
    }
    let reserved = u16::from_le_bytes([ico_bytes[0], ico_bytes[1]]);
    let ico_type = u16::from_le_bytes([ico_bytes[2], ico_bytes[3]]);
    let count = u16::from_le_bytes([ico_bytes[4], ico_bytes[5]]) as usize;
    if reserved != 0 || ico_type != 1 || count == 0 || ico_bytes.len() < 6 + count * 16 {
        return None;
    }

    let mut best: Option<(i32, u32, u32, usize, usize)> = None;
    for i in 0..count {
        let o = 6 + i * 16;
        let w = if ico_bytes[o] == 0 {
            256u32
        } else {
            ico_bytes[o] as u32
        };
        let h = if ico_bytes[o + 1] == 0 {
            256u32
        } else {
            ico_bytes[o + 1] as u32
        };
        if w != h || w == 0 {
            continue;
        }
        let bpp = u16::from_le_bytes([ico_bytes[o + 6], ico_bytes[o + 7]]) as i32;
        let size = u32::from_le_bytes(ico_bytes[o + 8..o + 12].try_into().ok()?) as usize;
        let offset = u32::from_le_bytes(ico_bytes[o + 12..o + 16].try_into().ok()?) as usize;
        if offset.checked_add(size).is_none_or(|end| end > ico_bytes.len()) || size == 0 {
            continue;
        }
        // Preferimos 64x64 o 48x48 para taskbar/Alt+Tab nítidos sin exceso de memoria
        let pref = match w {
            64 => 600,
            48 => 550,
            128 => 500,
            32 => 450,
            256 => 400,
            _ => (w as i32).min(300),
        } + bpp;
        if best.is_none_or(|(b_score, _, _, _, _)| pref > b_score) {
            best = Some((pref, w, h, offset, size));
        }
    }

    let (_, w, h, offset, size) = best?;
    let payload = &ico_bytes[offset..offset + size];

    if payload.starts_with(b"\x89PNG\r\n\x1a\n") {
        return eframe::icon_data::from_png_bytes(payload).ok();
    }

    if payload.len() < 40 {
        return None;
    }
    let hdr_size = u32::from_le_bytes(payload[0..4].try_into().ok()?) as usize;
    let bpp = u16::from_le_bytes(payload[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(payload[16..20].try_into().ok()?);
    if hdr_size < 40 || bpp != 32 || compression != 0 {
        return None;
    }

    let px_count = (w as usize).checked_mul(h as usize)?;
    let px_bytes = px_count.checked_mul(4)?;
    if payload.len() < hdr_size.checked_add(px_bytes)? {
        return None;
    }

    let src = &payload[hdr_size..hdr_size + px_bytes];
    let mut rgba = vec![0u8; px_bytes];
    let row_bytes = (w as usize) * 4;
    for y in 0..(h as usize) {
        let src_row = (h as usize - 1 - y) * row_bytes;
        let dst_row = y * row_bytes;
        for x in 0..(w as usize) {
            let s = src_row + x * 4;
            let d = dst_row + x * 4;
            rgba[d] = src[s + 2];
            rgba[d + 1] = src[s + 1];
            rgba[d + 2] = src[s];
            rgba[d + 3] = src[s + 3];
        }
    }

    Some(eframe::egui::IconData {
        rgba,
        width: w,
        height: h,
    })
}

pub(crate) fn build_nfc_rgba_bytes() -> Vec<u8> {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - 15.5).abs();
            let dy = (y as f32 - 15.5).abs();
            let idx = (y * size + x) * 4;
            let corner = if dx > 10.5 && dy > 10.5 {
                ((dx - 10.5).powi(2) + (dy - 10.5).powi(2)).sqrt() <= 5.0
            } else {
                dx <= 15.5 && dy <= 15.5
            };
            if corner {
                let cx = x as f32 - 11.5;
                let cy = y as f32 - 15.5;
                let dist = (cx * cx + cy * cy).sqrt();
                let in_arc = cx >= -1.0
                    && ((3.2..=5.4).contains(&dist)
                        || (7.6..=9.8).contains(&dist)
                        || (12.0..=14.2).contains(&dist));
                let in_dot =
                    ((x as f32 - 10.5).powi(2) + (y as f32 - 15.5).powi(2)).sqrt() <= 2.1;
                if in_arc || in_dot {
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                    rgba[idx + 3] = 255;
                } else {
                    rgba[idx] = 59;
                    rgba[idx + 1] = 130;
                    rgba[idx + 2] = 246;
                    rgba[idx + 3] = 255;
                }
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_app_icon_fallback_and_custom_ico() {
        let fb = load_app_icon_data(b"invalid");
        assert_eq!((fb.width, fb.height), (32, 32));
        assert_eq!(fb.rgba.len(), 32 * 32 * 4);

        let ico_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/icons/sync/app.ico");
        if ico_path.exists() {
            let bytes = std::fs::read(&ico_path).expect("sync app.ico readable");
            let icon = load_app_icon_data(&bytes);
            assert_eq!((icon.width, icon.height), (64, 64));
            assert_eq!(icon.rgba.len(), 64 * 64 * 4);
        }
    }
}
