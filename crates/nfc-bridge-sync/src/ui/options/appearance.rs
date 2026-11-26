use super::super::icons::{glyph, paint_chevron_y, paint_glyph, semibold};
use super::super::state::SharedUiCore;
use super::super::theme::{parse_hex_rgb, rgb_to_hex, rgb_to_hsv, ThemePalette};
use super::super::widgets::{paint_color_hex_chip, paint_divider, switch_pill_btn};
use egui::{
    Align, Color32, CornerRadius, FontId, Id, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use std::sync::{Arc, Mutex};

pub(crate) fn render_opt_appearance(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        hdr,
        theme_lbl,
        lang_lbl,
        win_acc_title,
        win_acc_desc,
        custom_lbl,
        theme_mode,
        lang_val,
        win_acc_on,
        win_hex,
        custom_hex,
        lang_open,
    ) = {
        let g = core.lock().unwrap();
        (
            g.t("settings_appearance_header").to_uppercase(),
            g.t("settings_theme_label").to_string(),
            g.t("settings_language_label").to_string(),
            g.t("settings_win_accent_title").to_string(),
            g.t("settings_win_accent_desc").to_string(),
            g.t("settings_custom_color_label").to_string(),
            g.live.ui_settings.theme_mode.clone(),
            g.live.ui_settings.language.clone(),
            g.live.ui_settings.use_windows_accent,
            g.live.windows_accent_hex.clone(),
            g.live.ui_settings.custom_accent_hex.clone(),
            g.language_dropdown_open,
        )
    };

    super::options_section_header(ui, pal, &hdr, |_| {});

    egui::Frame::new()
        .fill(pal.surface_elevated)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // 1. Tema con iconos (Sistema, Oscuro, Claro)
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(theme_lbl)
                        .font(semibold(14.0))
                        .color(pal.on_surface),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    egui::Frame::new()
                        .fill(pal.surface)
                        .stroke(Stroke::new(1.0, pal.outline))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let choices = [
                                    ("system", glyph::CONTRAST, "settings_theme_system"),
                                    ("dark", glyph::DARK_MODE, "settings_theme_dark"),
                                    ("light", glyph::LIGHT_MODE, "settings_theme_light"),
                                ];
                                for (mode_k, ic, lbl_k) in choices {
                                    let lbl = { core.lock().unwrap().t(lbl_k).to_string() };
                                    let active = theme_mode == mode_k;
                                    let fg = if active {
                                        pal.on_accent
                                    } else {
                                        pal.on_surface_muted
                                    };
                                    let lbl_gal = ui.painter().layout_no_wrap(
                                        lbl.clone(),
                                        FontId::proportional(11.0),
                                        fg,
                                    );
                                    let w = (lbl_gal.size().x + 34.0).max(66.0);
                                    let (r, resp) =
                                        ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
                                    if active {
                                        ui.painter().rect_filled(
                                            r,
                                            CornerRadius::same(6),
                                            pal.accent,
                                        );
                                    }
                                    paint_glyph(
                                        ui.painter(),
                                        Pos2::new(r.left() + 14.0, r.center().y),
                                        ic,
                                        14.0,
                                        fg,
                                    );
                                    ui.painter().galley(
                                        Pos2::new(
                                            r.left() + 24.0,
                                            r.center().y - lbl_gal.size().y * 0.5,
                                        ),
                                        lbl_gal,
                                        fg,
                                    );
                                    if resp.clicked() {
                                        let mut g = core.lock().unwrap();
                                        g.live.ui_settings.theme_mode = mode_k.to_string();
                                        g.persist_ui_settings();
                                        super::super::repaint_all_viewports(ui.ctx());
                                    }
                                }
                            });
                        });
                });
            });

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            // 2. Dropdown de idioma personalizado
            let mut lang_btn_rect = Rect::NOTHING;
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(lang_lbl)
                        .font(semibold(14.0))
                        .color(pal.on_surface),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let cur_lbl = {
                        let g = core.lock().unwrap();
                        let k = match lang_val.as_str() {
                            "en" => "settings_language_en",
                            "es" => "settings_language_es",
                            _ => "settings_language_system",
                        };
                        g.t(k).to_string()
                    };
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(150.0, 32.0), Sense::click());
                    lang_btn_rect = r;
                    ui.painter().rect(
                        r,
                        CornerRadius::same(8),
                        pal.surface,
                        Stroke::new(1.0, if lang_open { pal.accent } else { pal.outline }),
                        StrokeKind::Inside,
                    );
                    let cur_gal = ui.painter().layout_no_wrap(
                        cur_lbl,
                        FontId::proportional(12.0),
                        pal.on_surface,
                    );
                    ui.painter().galley(
                        Pos2::new(r.left() + 12.0, r.center().y - cur_gal.size().y * 0.5),
                        cur_gal,
                        pal.on_surface,
                    );
                    paint_chevron_y(
                        ui.painter(),
                        Pos2::new(r.right() - 15.0, r.center().y),
                        lang_open,
                        15.0,
                        pal.on_surface_muted,
                    );
                    if resp.clicked() {
                        let mut g = core.lock().unwrap();
                        g.language_dropdown_open = !g.language_dropdown_open;
                    }
                });
            });

            if lang_open && lang_btn_rect.is_positive() {
                egui::Area::new(Id::unique("nb_lang_menu_area"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(Pos2::new(
                        lang_btn_rect.right() - 166.0,
                        lang_btn_rect.bottom() + 4.0,
                    ))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::new()
                            .fill(pal.surface_card)
                            .stroke(Stroke::new(1.0, pal.outline))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::same(4))
                            .show(ui, |ui| {
                                ui.set_width(158.0);
                                let langs = [
                                    ("system", "settings_language_system"),
                                    ("en", "settings_language_en"),
                                    ("es", "settings_language_es"),
                                ];
                                for (lk, tk) in langs {
                                    let txt = { core.lock().unwrap().t(tk).to_string() };
                                    let is_sel = lang_val == lk;
                                    let (r, resp) = ui
                                        .allocate_exact_size(Vec2::new(158.0, 30.0), Sense::click());
                                    if is_sel {
                                        ui.painter().rect_filled(
                                            r,
                                            CornerRadius::same(6),
                                            pal.accent_soft,
                                        );
                                    } else if resp.hovered() {
                                        ui.painter().rect_filled(
                                            r,
                                            CornerRadius::same(6),
                                            pal.surface_elevated,
                                        );
                                    }
                                    let fg = if is_sel {
                                        pal.accent_on_soft
                                    } else {
                                        pal.on_surface
                                    };
                                    ui.painter().text(
                                        Pos2::new(r.left() + 10.0, r.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        txt,
                                        FontId::proportional(12.0),
                                        fg,
                                    );
                                    if is_sel {
                                        paint_glyph(
                                            ui.painter(),
                                            Pos2::new(r.right() - 14.0, r.center().y),
                                            glyph::CHECK,
                                            15.0,
                                            pal.accent,
                                        );
                                    }
                                    if resp.clicked() {
                                        let mut g = core.lock().unwrap();
                                        g.live.ui_settings.language = lk.to_string();
                                        g.language_dropdown_open = false;
                                        g.persist_ui_settings();
                                        super::super::repaint_all_viewports(ui.ctx());
                                    }
                                }
                            });
                    });
            }

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            // 3. Color de acento de Windows
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(win_acc_title)
                            .font(semibold(14.0))
                            .color(pal.on_surface),
                    );
                    ui.label(
                        RichText::new(win_acc_desc)
                            .size(11.0)
                            .color(pal.on_surface_muted),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let state_lbl = {
                        core.lock()
                            .unwrap()
                            .t(if win_acc_on { "state_on" } else { "state_off" })
                            .to_string()
                    };
                    if switch_pill_btn(ui, pal, &state_lbl, win_acc_on) {
                        let mut g = core.lock().unwrap();
                        g.live.ui_settings.use_windows_accent =
                            !g.live.ui_settings.use_windows_accent;
                        g.persist_ui_settings();
                        super::super::repaint_all_viewports(ui.ctx());
                    }
                    ui.add_space(6.0);
                    let (wr, wg, wb) = parse_hex_rgb(&win_hex);
                    paint_color_hex_chip(ui, pal, Color32::from_rgb(wr, wg, wb), &win_hex);
                });
            });

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            // 4. Color personalizado
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(custom_lbl)
                        .size(12.5)
                        .color(pal.on_surface_muted),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let presets = [
                        "#3B82F6", "#06B6D4", "#10B981", "#8B5CF6", "#EC4899", "#F59E0B",
                    ];
                    let cur_upper = custom_hex.to_uppercase();
                    let matched_preset = !win_acc_on && presets.iter().any(|p| *p == cur_upper);
                    let custom_active = !win_acc_on && !matched_preset;

                    let (eye_r, eye_resp) =
                        ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                    if custom_active {
                        ui.painter().circle_filled(eye_r.center(), 14.0, pal.accent);
                        ui.painter()
                            .circle_stroke(eye_r.center(), 15.5, Stroke::new(2.5, pal.accent));
                        ui.painter().circle_stroke(
                            eye_r.center(),
                            13.0,
                            Stroke::new(1.5, pal.surface_card),
                        );
                        paint_glyph(
                            ui.painter(),
                            eye_r.center(),
                            glyph::COLORIZE,
                            15.0,
                            pal.on_accent,
                        );
                    } else {
                        ui.painter().circle(
                            eye_r.center(),
                            13.0,
                            pal.surface,
                            Stroke::new(
                                1.0,
                                if eye_resp.hovered() {
                                    pal.accent
                                } else {
                                    pal.outline
                                },
                            ),
                        );
                        paint_glyph(
                            ui.painter(),
                            eye_r.center(),
                            glyph::COLORIZE,
                            15.0,
                            pal.on_surface,
                        );
                    }
                    if eye_resp.clicked() {
                        let mut g = core.lock().unwrap();
                        let (r, gr, b) = parse_hex_rgb(&g.live.ui_settings.custom_accent_hex);
                        let (h, s, v) = rgb_to_hsv(r, gr, b);
                        g.hsv_h = if h == 0.0 && s == 0.0 { 217.0 } else { h };
                        g.hsv_s = s.max(0.05);
                        g.hsv_v = v.max(0.15);
                        g.hsv_hex_buf = rgb_to_hex(r, gr, b);
                        g.color_picker_anchor = eye_r.left_bottom();
                        g.color_picker_open = !g.color_picker_open;
                    }

                    ui.add_space(6.0);
                    for hex in presets.iter().rev() {
                        let (r, gr, b) = parse_hex_rgb(hex);
                        let c = Color32::from_rgb(r, gr, b);
                        let sel = !win_acc_on && cur_upper == *hex;
                        let (cr, cresp) =
                            ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                        ui.painter().circle_filled(cr.center(), 11.0, c);
                        if sel {
                            ui.painter()
                                .circle_stroke(cr.center(), 13.5, Stroke::new(2.2, c));
                        }
                        if cresp.clicked() {
                            let mut g = core.lock().unwrap();
                            g.live.ui_settings.use_windows_accent = false;
                            g.live.ui_settings.custom_accent_hex = (*hex).to_string();
                            g.persist_ui_settings();
                            super::super::repaint_all_viewports(ui.ctx());
                        }
                        ui.add_space(4.0);
                    }
                });
            });
        });
}
