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
    pub const CHECK: char = '\u{e5ca}';
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

static MATERIAL_SYMBOLS_TTF: &[u8] = include_bytes!("../../assets/material-symbols.ttf");

pub fn icon_family() -> FontFamily {
    FontFamily::Name("icons".into())
}

pub fn semibold_family() -> FontFamily {
    FontFamily::Name("semibold".into())
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, semibold_family())
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

    if let Some(segoe_bytes) = load_windows_font(&["segoeui.ttf", "SegUIVar.ttf"]) {
        fonts.font_data.insert(
            "segoe_ui".to_owned(),
            Arc::new(FontData::from_owned(segoe_bytes)),
        );
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe_ui".to_owned());
        }
    }

    if let Some(sb_bytes) = load_windows_font(&["seguisb.ttf", "segoeuib.ttf", "segoeui.ttf"]) {
        fonts.font_data.insert(
            "segoe_ui_sb".to_owned(),
            Arc::new(FontData::from_owned(sb_bytes)),
        );
        fonts.families.insert(
            semibold_family(),
            vec!["segoe_ui_sb".to_owned(), "material_symbols".to_owned()],
        );
    } else {
        fonts.families.insert(
            semibold_family(),
            vec!["material_symbols".to_owned()],
        );
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

pub fn paint_chevron_y(painter: &egui::Painter, center: Pos2, up: bool, size: f32, color: Color32) {
    let dx = size * 0.25;
    let dy = size * 0.14 * if up { -1.0 } else { 1.0 };
    let stroke = Stroke::new((size * 0.105).max(1.4), color);
    painter.line_segment(
        [
            Pos2::new(center.x - dx, center.y - dy),
            Pos2::new(center.x, center.y + dy),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(center.x, center.y + dy),
            Pos2::new(center.x + dx, center.y - dy),
        ],
        stroke,
    );
}

pub fn paint_refresh_spin(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    angle: f32,
    color: Color32,
) {
    let segs = 20usize;
    let start_a = angle + std::f32::consts::FRAC_PI_4;
    let sweep = std::f32::consts::PI * 1.50;
    let mut pts = Vec::with_capacity(segs + 1);
    for i in 0..=segs {
        let a = start_a + sweep * (i as f32 / segs as f32);
        pts.push(Pos2::new(
            center.x + a.cos() * radius,
            center.y + a.sin() * radius,
        ));
    }
    painter.add(egui::Shape::line(pts, Stroke::new(1.45, color)));

    let tip_a = start_a + sweep;
    let norm = Vec2::new(tip_a.cos(), tip_a.sin());
    let dir = Vec2::new(-tip_a.sin(), tip_a.cos());
    let tip = center + norm * radius + dir * 2.0;
    let base = center + norm * radius - dir * 1.3;
    let p1 = base - norm * 2.35;
    let p2 = base + norm * 2.35;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, p1, p2],
        color,
        Stroke::NONE,
    ));
}

pub fn paint_device_os_icon(
    painter: &egui::Painter,
    rect: Rect,
    os: &str,
    color: Color32,
    eye_bg: Color32,
) {
    let c = rect.center();
    if os.eq_ignore_ascii_case("ios") {
        paint_glyph(painter, c, glyph::PHONE_IPHONE, rect.height() * 0.9, color);
    } else {
        let s = rect.height().min(rect.width()) / 24.0;
        let ox = rect.left() + (rect.width() - 24.0 * s) * 0.5;
        let oy = rect.top() + (rect.height() - 24.0 * s) * 0.5;
        let cx = ox + 12.0 * s;
        let base_y = oy + 17.0 * s;
        let rx = 10.4 * s;
        let ry = 8.4 * s;
        let mut pts = Vec::with_capacity(22);
        for i in 0..=20 {
            let ang = std::f32::consts::PI * (i as f32 / 20.0);
            pts.push(Pos2::new(cx - ang.cos() * rx, base_y - ang.sin() * ry));
        }
        painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        let ant_stroke = Stroke::new((1.55 * s).max(1.1), color);
        painter.line_segment(
            [
                Pos2::new(ox + 8.9 * s, oy + 9.2 * s),
                Pos2::new(ox + 6.9 * s, oy + 5.7 * s),
            ],
            ant_stroke,
        );
        painter.line_segment(
            [
                Pos2::new(ox + 15.1 * s, oy + 9.2 * s),
                Pos2::new(ox + 17.1 * s, oy + 5.7 * s),
            ],
            ant_stroke,
        );
        painter.circle_filled(
            Pos2::new(ox + 8.3 * s, oy + 13.7 * s),
            (1.35 * s).max(0.95),
            eye_bg,
        );
        painter.circle_filled(
            Pos2::new(ox + 15.7 * s, oy + 13.7 * s),
            (1.35 * s).max(0.95),
            eye_bg,
        );
    }
}

pub fn paint_update_down_badge(
    painter: &egui::Painter,
    btn_rect: Rect,
    bg: Color32,
    accent: Color32,
) {
    let badge_center = Pos2::new(btn_rect.right() - 7.5, btn_rect.bottom() - 7.5);
    painter.circle_filled(badge_center, 6.2, bg);
    painter.circle_stroke(badge_center, 6.2, Stroke::new(1.0, bg));
    painter.line_segment(
        [
            Pos2::new(badge_center.x, badge_center.y - 3.2),
            Pos2::new(badge_center.x, badge_center.y + 2.8),
        ],
        Stroke::new(1.7, accent),
    );
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
