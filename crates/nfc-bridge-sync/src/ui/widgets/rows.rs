use super::super::icons::{glyph, paint_glyph, semibold};
use super::super::theme::ThemePalette;
use egui::{Align, Color32, CornerRadius, FontId, Layout, Pos2, RichText, Sense, Stroke, StrokeKind, Vec2};

pub(crate) fn info_copy_tile_w(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    w: f32,
    label: &str,
    value: &str,
    copied: bool,
) -> bool {
    info_copy_tile_w_col(ui, pal, w, label, value, copied, pal.on_surface)
}

pub(crate) fn info_copy_tile_w_col(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    w: f32,
    label: &str,
    value: &str,
    copied: bool,
    val_col: Color32,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 50.0), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        pal.surface_elevated,
        Stroke::new(
            1.0,
            if resp.hovered() {
                pal.accent_border
            } else {
                pal.outline
            },
        ),
        StrokeKind::Inside,
    );
    ui.painter().text(
        Pos2::new(r.left() + 12.0, r.top() + 8.0),
        egui::Align2::LEFT_TOP,
        label,
        FontId::proportional(11.0),
        pal.on_surface_muted,
    );
    ui.painter().text(
        Pos2::new(r.left() + 12.0, r.top() + 25.0),
        egui::Align2::LEFT_TOP,
        value,
        FontId::monospace(12.0),
        val_col,
    );
    paint_glyph(
        ui.painter(),
        Pos2::new(r.right() - 16.0, r.top() + 32.0),
        if copied {
            glyph::CHECK
        } else {
            glyph::CONTENT_COPY
        },
        14.0,
        if copied {
            pal.led_green
        } else if resp.hovered() {
            pal.accent
        } else {
            pal.on_surface_muted
        },
    );
    resp.clicked()
}

pub(crate) fn info_kv_row(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    value: &str,
    elevated_bg: bool,
) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        if elevated_bg {
            pal.surface_elevated
        } else {
            pal.surface
        },
        Stroke::new(1.0, pal.outline),
        StrokeKind::Inside,
    );
    ui.painter().text(
        Pos2::new(r.left() + 12.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        pal.on_surface_muted,
    );
    ui.painter().text(
        Pos2::new(r.right() - 12.0, r.center().y),
        egui::Align2::RIGHT_CENTER,
        value,
        FontId::monospace(12.0),
        pal.on_surface,
    );
}

pub(crate) fn about_row(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    left: &str,
    right: &str,
    left_strong: bool,
    right_col: Color32,
) {
    ui.horizontal(|ui| {
        if left_strong {
            ui.label(RichText::new(left).font(semibold(14.0)).color(pal.on_surface));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(right).monospace().size(12.0).color(right_col));
            });
        } else {
            ui.label(RichText::new(left).size(11.0).color(pal.on_surface_muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(right).monospace().size(11.0).color(right_col));
            });
        }
    });
}
