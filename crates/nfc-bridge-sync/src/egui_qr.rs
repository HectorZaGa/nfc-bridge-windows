//! Renderizado QR nativo en `egui` con nivel de corrección de errores H e icono central
//! envuelto por un contorno en forma exacta con separación limpia sobre los módulos QR.

use crate::egui_icons::{glyph, paint_glyph};
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Vec2};
use qrcode::{Color as QrColor, EcLevel, QrCode};

pub fn paint_qr_box(
    painter: &Painter,
    box_rect: Rect,
    payload: &str,
    method: &str,
    accent: Color32,
) -> Result<(), String> {
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::H)
        .map_err(|e| e.to_string())?;

    painter.rect_filled(box_rect, CornerRadius::same(12), Color32::WHITE);

    let width = code.width();
    let colors = code.to_colors();
    let pad = 10.0_f32;
    let inner = box_rect.shrink(pad);
    let mod_size = inner.width() / (width as f32);
    let dark = Color32::from_rgb(18, 18, 18);

    for row in 0..width {
        for col in 0..width {
            if colors[row * width + col] == QrColor::Dark {
                let x0 = inner.left() + (col as f32) * mod_size;
                let y0 = inner.top() + (row as f32) * mod_size;
                let r = Rect::from_min_size(Pos2::new(x0, y0), Vec2::splat(mod_size + 0.35));
                painter.rect_filled(r, CornerRadius::ZERO, dark);
            }
        }
    }

    // Icono central más grande y grueso con contorno de forma exacta y separación blanca limpia
    let icon_char = match method {
        "tcp" => glyph::WIFI,
        "bluetooth" => glyph::BLUETOOTH,
        "wifi_direct" => glyph::WIFI_TETHERING,
        _ => glyph::LINK,
    };

    let center = box_rect.center();
    let icon_size = 34.0_f32;
    let contour_spread = 4.6_f32;
    let contour_font_size = icon_size + 6.0;

    // 16 pasos radiales + centro en blanco dibujan el contorno siguiendo exactamente la silueta del icono
    paint_glyph(painter, center, icon_char, contour_font_size, Color32::WHITE);
    for i in 0..16 {
        let ang = (i as f32) * (std::f32::consts::TAU / 16.0);
        let offset = Vec2::new(ang.cos() * contour_spread, ang.sin() * contour_spread);
        paint_glyph(
            painter,
            center + offset,
            icon_char,
            contour_font_size,
            Color32::WHITE,
        );
    }
    // Doble pasada ligera para dar mayor grosor visual al icono central
    paint_glyph(painter, center, icon_char, icon_size, accent);
    paint_glyph(
        painter,
        center + Vec2::new(0.45, 0.0),
        icon_char,
        icon_size,
        accent,
    );

    Ok(())
}
