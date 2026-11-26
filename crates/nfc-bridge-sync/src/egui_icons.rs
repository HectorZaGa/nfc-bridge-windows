//! Iconos vectoriales y fuente Material Symbols Outlined incrustada (13 KB offline)
//! para la interfaz nativa `egui` de NFC Bridge Sync y Tools Desktop.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use std::sync::Arc;

#[allow(dead_code)]
pub mod glyph {
    pub const ANDROID: char = '\u{e859}';
    pub const ARROW_BACK: char = '\u{e5c4}';
    pub const BLUETOOTH: char = '\u{e1a7}';
    pub const BLUETOOTH_DISABLED: char = '\u{e1a9}';
    pub const BLUETOOTH_SEARCHING: char = '\u{e60f}';
    pub const CABLE: char = '\u{efe6}';
    pub const CELL_TOWER: char = '\u{ebba}';
    pub const CHECK: char = '\u{e668}';
    pub const CHEVRON_RIGHT: char = '\u{e5cc}';
    pub const CLOSE: char = '\u{e5cd}';
    pub const COLORIZE: char = '\u{e3b8}';
    pub const CONTACTLESS: char = '\u{ea71}';
    pub const CONTENT_COPY: char = '\u{e14d}';
    pub const CONTRAST: char = '\u{eb37}';
    pub const DARK_MODE: char = '\u{e51c}';
    pub const DELETE: char = '\u{e92e}';
    pub const DESKTOP_WINDOWS: char = '\u{e30c}';
    pub const DOCK_TO_LEFT: char = '\u{f7e5}';
    pub const EDIT: char = '\u{f097}';
    pub const EXPAND_MORE: char = '\u{e5cf}';
    pub const HUB: char = '\u{e9f4}';
    pub const INFO: char = '\u{e88e}';
    pub const LAN: char = '\u{eb2f}';
    pub const LIGHT_MODE: char = '\u{e518}';
    pub const LINK: char = '\u{e250}';
    pub const LINK_OFF: char = '\u{e16f}';
    pub const NFC: char = '\u{e1bb}';
    pub const OPEN_IN_NEW: char = '\u{e89e}';
    pub const PALETTE: char = '\u{e40a}';
    pub const PAUSE: char = '\u{e034}';
    pub const PHONE_IPHONE: char = '\u{f2da}';
    pub const PHONELINK: char = '\u{e326}';
    pub const PHONELINK_OFF: char = '\u{f7a5}';
    pub const PLAY_ARROW: char = '\u{e037}';
    pub const POWER_SETTINGS_NEW: char = '\u{f8c7}';
    pub const QR_CODE: char = '\u{ef6b}';
    pub const REFRESH: char = '\u{e5d5}';
    pub const REMOVE: char = '\u{e15b}';
    pub const RESTART_ALT: char = '\u{f053}';
    pub const ROUTER: char = '\u{e328}';
    pub const SETTINGS: char = '\u{e8b8}';
    pub const SMARTPHONE: char = '\u{e7ba}';
    pub const SWAP_HORIZ: char = '\u{e8d4}';
    pub const SYSTEM_UPDATE_ALT: char = '\u{e8d7}';
    pub const TUNE: char = '\u{e429}';
    pub const USB: char = '\u{e1e0}';
    pub const USB_OFF: char = '\u{e4fa}';
    pub const WARNING: char = '\u{f083}';
    pub const WIFI: char = '\u{e63e}';
    pub const WIFI_OFF: char = '\u{e648}';
    pub const WIFI_TETHERING: char = '\u{e1e2}';
}

static MATERIAL_SYMBOLS_TTF: &[u8] = include_bytes!("../assets/material-symbols.ttf");

pub fn icon_family() -> FontFamily {
    FontFamily::Name("icons".into())
}

pub fn icon_rich(ch: char, size: f32, color: Color32) -> RichText {
    let mut s = String::with_capacity(4);
    s.push(ch);
    RichText::new(s)
        .font(FontId::new(size, icon_family()))
        .color(color)
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "material_symbols".to_owned(),
        Arc::new(FontData::from_static(MATERIAL_SYMBOLS_TTF)),
    );

    if let Some(segoe_bytes) = load_windows_font(&["SegUIVar.ttf", "segoeui.ttf"]) {
        fonts.font_data.insert(
            "segoe_ui".to_owned(),
            Arc::new(FontData::from_owned(segoe_bytes)),
        );
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe_ui".to_owned());
        }
    }

    if let Some(mono_bytes) = load_windows_font(&["CascadiaMono.ttf", "consola.ttf"]) {
        fonts.font_data.insert(
            "win_mono".to_owned(),
            Arc::new(FontData::from_owned(mono_bytes)),
        );
        if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
            list.insert(0, "win_mono".to_owned());
        }
    }

    fonts
        .families
        .insert(icon_family(), vec!["material_symbols".to_owned()]);

    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.push("material_symbols".to_owned());
    }

    ctx.set_fonts(fonts);
}

fn load_windows_font(candidates: &[&str]) -> Option<Vec<u8>> {
    let win_dir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
    let fonts_dir = win_dir.join("Fonts");
    for name in candidates {
        let path = fonts_dir.join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            if !bytes.is_empty() {
                return Some(bytes);
            }
        }
    }
    None
}

/// Dibuja un icono Material Symbol centrado exactamente dentro de `rect`.
pub fn paint_glyph(painter: &egui::Painter, center: Pos2, ch: char, size: f32, color: Color32) {
    let mut s = String::with_capacity(4);
    s.push(ch);
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        s,
        FontId::new(size, icon_family()),
        color,
    );
}

/// Pinta el icono oficial de Android (cabeza + antenas + ojos) o Apple (silueta estilizada) en pequeño.
pub fn paint_device_os_icon(painter: &egui::Painter, rect: Rect, os: &str, color: Color32) {
    let c = rect.center();
    if os.eq_ignore_ascii_case("ios") {
        paint_glyph(painter, c, glyph::PHONE_IPHONE, rect.height() * 0.86, color);
    } else {
        // Cabeza Android limpia estilo Stitch (arco superior + base plana + 2 antenas + 2 ojos)
        let s = rect.height().min(rect.width()) / 24.0;
        let cx = c.x;
        let cy = c.y + 3.5 * s;
        let rx = 7.2 * s;
        let mut pts = Vec::with_capacity(18);
        for i in 0..=16 {
            let ang = std::f32::consts::PI * (i as f32 / 16.0);
            pts.push(Pos2::new(cx - ang.cos() * rx, cy - ang.sin() * (6.6 * s)));
        }
        painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        // Antenas
        painter.line_segment(
            [Pos2::new(cx - 3.0 * s, cy - 5.3 * s), Pos2::new(cx - 4.8 * s, cy - 8.4 * s)],
            Stroke::new((1.5 * s).max(1.1), color),
        );
        painter.line_segment(
            [Pos2::new(cx + 3.0 * s, cy - 5.3 * s), Pos2::new(cx + 4.8 * s, cy - 8.4 * s)],
            Stroke::new((1.5 * s).max(1.1), color),
        );
        // Ojos recortados en color de tarjeta (oscuros sobre claro o viceversa)
        let eye_bg = if color.r() > 140 {
            Color32::from_rgb(38, 38, 38)
        } else {
            Color32::from_rgb(236, 238, 242)
        };
        painter.circle_filled(Pos2::new(cx - 2.6 * s, cy - 2.2 * s), (1.05 * s).max(1.0), eye_bg);
        painter.circle_filled(Pos2::new(cx + 2.6 * s, cy - 2.2 * s), (1.05 * s).max(1.0), eye_bg);
    }
}

/// Dibuja la miniatura de badge de actualización (flecha abajo dentro de cápsula) sobre la esquina del icono Settings.
pub fn paint_update_down_badge(painter: &egui::Painter, btn_rect: Rect, bg: Color32, accent: Color32) {
    let badge_center = Pos2::new(btn_rect.right() - 7.5, btn_rect.bottom() - 7.5);
    painter.circle_filled(badge_center, 6.2, bg);
    painter.circle_stroke(badge_center, 6.2, Stroke::new(1.0, bg));
    let arrow = [
        Pos2::new(badge_center.x, badge_center.y - 3.2),
        Pos2::new(badge_center.x, badge_center.y + 2.8),
    ];
    painter.line_segment(arrow, Stroke::new(1.7, accent));
    painter.line_segment(
        [
            Pos2::new(badge_center.x - 2.4, badge_center.y + 0.6),
            Pos2::new(badge_center.x, badge_center.y + 3.1),
        ],
        Stroke::new(1.7, accent),
    );
    painter.line_segment(
        [
            Pos2::new(badge_center.x + 2.4, badge_center.y + 0.6),
            Pos2::new(badge_center.x, badge_center.y + 3.1),
        ],
        Stroke::new(1.7, accent),
    );
}

#[allow(dead_code)]
fn _unused_vec(_v: Vec2) {}
