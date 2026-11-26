use super::super::icons::{cap_center_offset_y, paint_glyph};
use super::super::theme::ThemePalette;
use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

fn paint_label_cap_centered(
    painter: &egui::Painter,
    x: f32,
    center_y: f32,
    label: &str,
    font: FontId,
    fg: Color32,
) {
    let dy = cap_center_offset_y(painter, &font);
    let gal = painter.layout_no_wrap(label.to_string(), font, fg);
    painter.galley(Pos2::new(x, center_y - dy), gal, fg);
}

pub(crate) fn paint_chrome_btn(
    painter: &egui::Painter,
    rect: Rect,
    resp: &egui::Response,
    ic: char,
    danger: bool,
    pal: &ThemePalette,
) {
    if resp.hovered() {
        let bg = if danger {
            pal.red_soft
        } else {
            pal.surface_elevated
        };
        painter.rect_filled(rect, CornerRadius::same(8), bg);
    }
    let fg = if resp.hovered() && danger {
        pal.red_disconnect
    } else if resp.hovered() {
        pal.on_surface
    } else {
        pal.on_surface_muted
    };
    paint_glyph(painter, rect.center(), ic, 18.0, fg);
}

pub(crate) fn toggle_state_btn(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    ic: char,
    label: &str,
    state_txt: &str,
    on: bool,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        pal.surface_elevated,
        Stroke::new(1.0, if resp.hovered() { pal.accent } else { pal.outline }),
        StrokeKind::Inside,
    );
    paint_glyph(
        ui.painter(),
        Pos2::new(r.left() + 20.0, r.center().y),
        ic,
        16.0,
        if on { pal.accent } else { pal.on_surface_muted },
    );
    paint_label_cap_centered(
        ui.painter(),
        r.left() + 34.0,
        r.center().y,
        label,
        FontId::proportional(12.0),
        pal.on_surface,
    );
    let state_font = FontId::monospace(10.0);
    let state_dy = cap_center_offset_y(ui.painter(), &state_font);
    let state_gal = ui.painter().layout_no_wrap(
        state_txt.to_string(),
        state_font,
        if on { pal.led_green } else { pal.on_surface_muted },
    );
    ui.painter().galley(
        Pos2::new(r.right() - 12.0 - state_gal.size().x, r.center().y - state_dy),
        state_gal,
        if on { pal.led_green } else { pal.on_surface_muted },
    );
    resp.clicked()
}

pub(crate) fn primary_cta_btn(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    ic: char,
    label: &str,
    active_style: bool,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
    let bg = if active_style {
        if resp.hovered() {
            pal.accent.gamma_multiply(0.92)
        } else {
            pal.accent
        }
    } else {
        pal.surface_elevated
    };
    let fg = if active_style {
        pal.on_accent
    } else {
        pal.on_surface_muted
    };
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        bg,
        if active_style {
            Stroke::NONE
        } else {
            Stroke::new(1.0, if resp.hovered() { pal.accent } else { pal.outline })
        },
        StrokeKind::Inside,
    );

    let font = FontId::proportional(14.0);
    let dy = cap_center_offset_y(ui.painter(), &font);
    let gal = ui.painter().layout_no_wrap(label.to_string(), font, fg);
    let total_w = 22.0 + gal.size().x;
    let x0 = r.center().x - total_w * 0.5;
    paint_glyph(ui.painter(), Pos2::new(x0 + 8.0, r.center().y), ic, 16.0, fg);
    ui.painter()
        .galley(Pos2::new(x0 + 22.0, r.center().y - dy), gal, fg);
    resp.clicked()
}

struct CenteredIconBtnStyle {
    bg_normal: Color32,
    bg_hover: Color32,
    border_normal: Stroke,
    border_hover: Stroke,
    icon_color: Color32,
    fg: Color32,
}

fn centered_icon_btn(
    ui: &mut egui::Ui,
    ic: char,
    label: &str,
    st: CenteredIconBtnStyle,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let hovered = resp.hovered();
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        if hovered { st.bg_hover } else { st.bg_normal },
        if hovered {
            st.border_hover
        } else {
            st.border_normal
        },
        StrokeKind::Inside,
    );
    let font = FontId::proportional(12.0);
    let dy = cap_center_offset_y(ui.painter(), &font);
    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, st.fg);
    let total_w = 22.0 + gal.size().x;
    let x0 = r.center().x - total_w * 0.5;
    paint_glyph(
        ui.painter(),
        Pos2::new(x0 + 8.0, r.center().y),
        ic,
        16.0,
        st.icon_color,
    );
    ui.painter()
        .galley(Pos2::new(x0 + 22.0, r.center().y - dy), gal, st.fg);
    resp.clicked()
}

pub(crate) fn secondary_btn(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    ic: char,
    label: &str,
) -> bool {
    let border = Stroke::new(1.0, pal.outline);
    centered_icon_btn(
        ui,
        ic,
        label,
        CenteredIconBtnStyle {
            bg_normal: pal.surface_elevated,
            bg_hover: pal.outline,
            border_normal: border,
            border_hover: border,
            icon_color: pal.on_surface_muted,
            fg: pal.on_surface,
        },
    )
}

pub(crate) fn secondary_btn_accent_icon(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    ic: char,
    label: &str,
) -> bool {
    centered_icon_btn(
        ui,
        ic,
        label,
        CenteredIconBtnStyle {
            bg_normal: pal.surface_elevated,
            bg_hover: pal.surface_elevated,
            border_normal: Stroke::new(1.0, pal.outline),
            border_hover: Stroke::new(1.0, pal.accent),
            icon_color: pal.accent,
            fg: pal.on_surface,
        },
    )
}

pub(crate) fn small_ghost_btn(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    ic: char,
    label: &str,
) -> bool {
    let font = FontId::proportional(12.0);
    let dy = cap_center_offset_y(ui.painter(), &font);
    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, pal.on_surface);
    let w = gal.size().x + 32.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(r, CornerRadius::same(8), pal.surface_elevated);
    }
    paint_glyph(
        ui.painter(),
        Pos2::new(r.left() + 12.0, r.center().y),
        ic,
        15.0,
        pal.on_surface_muted,
    );
    ui.painter()
        .galley(Pos2::new(r.left() + 24.0, r.center().y - dy), gal, pal.on_surface);
    resp.clicked()
}

pub(crate) fn small_accent_btn(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    w: f32,
    h: f32,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    ui.painter().rect_filled(
        r,
        CornerRadius::same(8),
        if resp.hovered() {
            pal.accent.gamma_multiply(0.92)
        } else {
            pal.accent
        },
    );
    let font = FontId::proportional(12.0);
    let dy = cap_center_offset_y(ui.painter(), &font);
    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, pal.on_accent);
    ui.painter().galley(
        Pos2::new(r.center().x - gal.size().x * 0.5, r.center().y - dy),
        gal,
        pal.on_accent,
    );
    resp.clicked()
}

pub(crate) fn small_secondary_btn(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    w: f32,
    h: f32,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        if resp.hovered() {
            pal.surface
        } else {
            pal.surface_elevated
        },
        Stroke::new(1.0, pal.outline),
        StrokeKind::Inside,
    );
    let font = FontId::proportional(12.0);
    let dy = cap_center_offset_y(ui.painter(), &font);
    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, pal.on_surface);
    ui.painter().galley(
        Pos2::new(r.center().x - gal.size().x * 0.5, r.center().y - dy),
        gal,
        pal.on_surface,
    );
    resp.clicked()
}

pub(crate) fn small_danger_btn(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    w: f32,
    h: f32,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        pal.red_soft,
        Stroke::new(1.0, pal.red_border),
        StrokeKind::Inside,
    );
    let font = FontId::proportional(12.0);
    let dy = cap_center_offset_y(ui.painter(), &font);
    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, pal.red_disconnect);
    ui.painter().galley(
        Pos2::new(r.center().x - gal.size().x * 0.5, r.center().y - dy),
        gal,
        pal.red_disconnect,
    );
    resp.clicked()
}

pub(crate) fn switch_pill_btn(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    label: &str,
    on: bool,
) -> bool {
    let fg = if on {
        pal.accent_on_soft
    } else {
        pal.on_surface_muted
    };
    let font = FontId::proportional(11.0);
    let dy = cap_center_offset_y(ui.painter(), &font);
    let gal = ui.painter().layout_no_wrap(label.to_string(), font, fg);
    let w = (gal.size().x + 20.0).max(74.0);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        if on { pal.accent_soft } else { pal.surface },
        Stroke::new(1.0, if on { pal.accent_border } else { pal.outline }),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        Pos2::new(r.center().x - gal.size().x * 0.5, r.center().y - dy),
        gal,
        fg,
    );
    resp.clicked()
}
