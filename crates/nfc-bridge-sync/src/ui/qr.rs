//! Renderizado QR nativo en `egui` con nivel de corrección de errores M (módulos amplios)
//! e ícono central envuelto por contorno exacto centrado por bounding-box geométrico (`mesh_bounds`).

use super::icons::icon_family;
use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Vec2};
use qrcode::{Color as QrColor, EcLevel, QrCode};

pub fn paint_qr_box(
    painter: &Painter,
    box_rect: Rect,
    payload: &str,
    method: &str,
    accent: Color32,
) -> Result<(), String> {
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M)
        .map_err(|e| e.to_string())?;

    painter.rect_filled(box_rect, CornerRadius::same(12), Color32::WHITE);

    let width = code.width();
    let colors = code.to_colors();
    // Zona tranquila ISO estándar de 4 módulos por lado + margen exterior para esquinas redondeadas
    let quiet = 4usize;
    let total_mods = (width + quiet * 2) as f32;
    let inner = box_rect.shrink(4.0);
    let mod_size = inner.width() / total_mods;
    let dark = Color32::BLACK;

    for row in 0..width {
        for col in 0..width {
            if colors[row * width + col] == QrColor::Dark {
                let x0 = inner.left() + ((col + quiet) as f32) * mod_size;
                let y0 = inner.top() + ((row + quiet) as f32) * mod_size;
                let x1 = inner.left() + ((col + quiet + 1) as f32) * mod_size;
                let y1 = inner.top() + ((row + quiet + 1) as f32) * mod_size;
                let r = Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1 + 0.05, y1 + 0.05));
                painter.rect_filled(r, CornerRadius::ZERO, dark);
            }
        }
    }

    let icon_char = match method {
        "tcp" => super::icons::glyph::WIFI,
        "bluetooth" => super::icons::glyph::BLUETOOTH,
        "wifi_direct" => super::icons::glyph::WIFI_TETHERING,
        _ => super::icons::glyph::LINK,
    };

    let center = box_rect.center();
    let icon_size = 28.0_f32;
    let contour_font_size = icon_size + 5.0;
    let contour_spread = 3.2_f32;
    let mut glyph_str = String::with_capacity(4);
    glyph_str.push(icon_char);

    let contour_gal = painter.layout_no_wrap(
        glyph_str.clone(),
        FontId::new(contour_font_size, icon_family()),
        Color32::WHITE,
    );
    let contour_origin = center - contour_gal.mesh_bounds.center().to_vec2();
    painter.galley(contour_origin, contour_gal.clone(), Color32::WHITE);
    for i in 0..24 {
        let ang = (i as f32) * (std::f32::consts::TAU / 24.0);
        let offset = Vec2::new(ang.cos() * contour_spread, ang.sin() * contour_spread);
        painter.galley(contour_origin + offset, contour_gal.clone(), Color32::WHITE);
    }

    let fg_gal = painter.layout_no_wrap(
        glyph_str,
        FontId::new(icon_size, icon_family()),
        accent,
    );
    let fg_origin = center - fg_gal.mesh_bounds.center().to_vec2();
    painter.galley(fg_origin, fg_gal, accent);

    Ok(())
}
