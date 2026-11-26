use super::super::theme::ThemePalette;
use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

pub(crate) fn paint_pill(
    ui: &egui::Ui,
    center_left: Pos2,
    text: &str,
    font_size: f32,
    mono: bool,
    translucent: bool,
    pal: &ThemePalette,
) -> f32 {
    let font = if mono {
        FontId::monospace(font_size)
    } else {
        FontId::proportional(font_size)
    };
    let gal = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, pal.on_surface_muted);
    let w = gal.size().x + 16.0;
    let rect = Rect::from_min_size(Pos2::new(center_left.x, center_left.y - 10.5), Vec2::new(w, 21.0));
    let bg = if translucent {
        pal.build_badge_bg
    } else {
        pal.surface_elevated
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(11),
        bg,
        Stroke::new(1.0, pal.outline),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        Pos2::new(rect.left() + 8.0, rect.center().y - gal.size().y * 0.5),
        gal,
        pal.on_surface_muted,
    );
    rect.right()
}

pub(crate) fn paint_inline_capsule(
    ui: &mut egui::Ui,
    text: &str,
    mono: bool,
    pal: &ThemePalette,
) {
    let font = if mono {
        FontId::monospace(10.0)
    } else {
        FontId::proportional(10.0)
    };
    let gal = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, pal.on_surface_muted);
    let (r, _) = ui.allocate_exact_size(Vec2::new(gal.size().x + 14.0, 19.0), Sense::hover());
    ui.painter().rect(
        r,
        CornerRadius::same(10),
        pal.surface,
        Stroke::new(1.0, pal.outline),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        Pos2::new(r.left() + 7.0, r.center().y - gal.size().y * 0.5),
        gal,
        pal.on_surface_muted,
    );
}

pub(crate) fn paint_status_badge(
    ui: &mut egui::Ui,
    text: &str,
    active: bool,
    pal: &ThemePalette,
) {
    let fg = if active {
        pal.accent_on_soft
    } else {
        pal.on_surface_muted
    };
    let bg = if active {
        pal.accent_soft
    } else {
        pal.surface_elevated
    };
    let stroke = if active {
        pal.accent_border
    } else {
        pal.outline
    };
    let gal = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(11.0), fg);
    let (r, _) = ui.allocate_exact_size(Vec2::new(gal.size().x + 16.0, 21.0), Sense::hover());
    ui.painter().rect(
        r,
        CornerRadius::same(11),
        bg,
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        Pos2::new(r.left() + 8.0, r.center().y - gal.size().y * 0.5),
        gal,
        fg,
    );
}

pub(crate) fn paint_divider(ui: &mut egui::Ui, pal: &ThemePalette) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, pal.outline));
}

pub(crate) fn paint_color_hex_chip(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    dot_col: Color32,
    hex: &str,
) {
    let gal = ui
        .painter()
        .layout_no_wrap(hex.to_string(), FontId::monospace(10.0), pal.on_surface_muted);
    let (r, _) = ui.allocate_exact_size(Vec2::new(gal.size().x + 26.0, 22.0), Sense::hover());
    ui.painter().rect(
        r,
        CornerRadius::same(11),
        pal.surface,
        Stroke::new(1.0, pal.outline),
        StrokeKind::Inside,
    );
    ui.painter()
        .circle_filled(Pos2::new(r.left() + 11.0, r.center().y), 4.5, dot_col);
    ui.painter().galley(
        Pos2::new(r.left() + 20.0, r.center().y - gal.size().y * 0.5),
        gal,
        pal.on_surface_muted,
    );
}
