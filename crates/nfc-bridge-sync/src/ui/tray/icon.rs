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
