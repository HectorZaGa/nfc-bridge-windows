use super::super::icons::paint_glyph;
use super::super::theme::ThemePalette;
use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

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
    ui.painter().text(
        Pos2::new(r.left() + 34.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        pal.on_surface,
    );
    ui.painter().text(
        Pos2::new(r.right() - 12.0, r.center().y),
        egui::Align2::RIGHT_CENTER,
        state_txt,
        FontId::monospace(10.0),
        if on {
            pal.led_green
        } else {
            pal.on_surface_muted
        },
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

    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(14.0), fg);
    let total_w = 22.0 + gal.size().x;
    let x0 = r.center().x - total_w * 0.5;
    paint_glyph(ui.painter(), Pos2::new(x0 + 8.0, r.center().y), ic, 16.0, fg);
    ui.painter()
        .galley(Pos2::new(x0 + 22.0, r.center().y - gal.size().y * 0.5), gal, fg);
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
    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(12.0), st.fg);
    let total_w = 22.0 + gal.size().x;
    let x0 = r.center().x - total_w * 0.5;
    paint_glyph(
        ui.painter(),
        Pos2::new(x0 + 8.0, r.center().y),
        ic,
        16.0,
        st.icon_color,
    );
    ui.painter().galley(
        Pos2::new(x0 + 22.0, r.center().y - gal.size().y * 0.5),
        gal,
        st.fg,
    );
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
    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(12.0), pal.on_surface);
    let w = gal.size().x + 32.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(r, CornerRadius::same(6), pal.surface_elevated);
    }
    paint_glyph(
        ui.painter(),
        Pos2::new(r.left() + 12.0, r.center().y),
        ic,
        15.0,
        pal.on_surface_muted,
    );
    ui.painter().galley(
        Pos2::new(r.left() + 24.0, r.center().y - gal.size().y * 0.5),
        gal,
        pal.on_surface,
    );
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
    ui.painter().text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
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
    ui.painter().text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
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
    ui.painter().text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
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
    let gal = ui
        .painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(11.0), fg);
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
        Pos2::new(r.center().x - gal.size().x * 0.5, r.center().y - gal.size().y * 0.5),
        gal,
        fg,
    );
    resp.clicked()
}
