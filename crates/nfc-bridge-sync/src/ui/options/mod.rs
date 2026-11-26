mod about;
mod appearance;
mod color_picker;
mod network;
mod updates;

use super::icons::{glyph, paint_glyph, semibold};
use super::state::{OptionsTab, SharedUiCore};
use super::theme::{apply_egui_visuals, apply_win32_dwm_to_title, ThemePalette};
use super::widgets::paint_chrome_btn;
use color_picker::render_hsv_color_popover;
use egui::{
    text::LayoutJob, Align, Color32, CornerRadius, Id, Layout, Margin, Pos2, Rect, Sense, Stroke,
    StrokeKind, TextFormat, Vec2, ViewportCommand, ViewportId,
};
use std::sync::{Arc, Mutex};

pub(crate) fn options_section_header(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    title: &str,
    add_right: impl FnOnce(&mut egui::Ui),
) {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), 22.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            let mut job = LayoutJob::default();
            job.append(
                title,
                0.0,
                TextFormat {
                    font_id: semibold(11.5),
                    color: pal.on_surface_muted,
                    extra_letter_spacing: 0.65,
                    ..Default::default()
                },
            );
            ui.label(job);
            ui.with_layout(Layout::right_to_left(Align::Center), add_right);
        },
    );
    ui.add_space(8.0);
}

pub(crate) fn render_options_viewport(
    core: &Arc<Mutex<SharedUiCore>>,
    opt_title: &str,
    ui: &mut egui::Ui,
) {
    let ctx = ui.ctx().clone();
    if ctx.input(|i| i.viewport().close_requested()) {
        core.lock().unwrap().options_open = false;
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    let (pal, needs_vis_before) = {
        let mut g = core.lock().unwrap();
        let pal = g.palette();
        let need_vis = g.last_visuals_pal != Some(pal);
        if need_vis {
            g.last_visuals_pal = Some(pal);
        }
        let dwm_key = (pal.is_dark, pal.outline);
        if g.last_dwm_opts_key != Some(dwm_key) {
            g.last_dwm_opts_key = Some(dwm_key);
            apply_win32_dwm_to_title(opt_title, &pal);
        }
        (pal, need_vis)
    };
    if needs_vis_before {
        apply_egui_visuals(&ctx, &pal);
    }

    egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // Custom Options Title Bar (44 px)
            let (tb_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::hover());
            ui.painter()
                .rect_filled(tb_rect, CornerRadius::ZERO, pal.surface_card);
            ui.painter().line_segment(
                [tb_rect.left_bottom(), tb_rect.right_bottom()],
                Stroke::new(1.0, pal.outline),
            );

            paint_glyph(
                ui.painter(),
                Pos2::new(tb_rect.left() + 22.0, tb_rect.center().y),
                glyph::TUNE,
                17.0,
                pal.accent,
            );
            let title_lbl = {
                let g = core.lock().unwrap();
                if g.updates.app_product == "tools" {
                    g.t("title_options_tools").to_string()
                } else {
                    g.t("title_options_sync").to_string()
                }
            };
            ui.painter().text(
                Pos2::new(tb_rect.left() + 38.0, tb_rect.center().y),
                egui::Align2::LEFT_CENTER,
                title_lbl,
                semibold(16.0),
                pal.on_surface,
            );

            let right = tb_rect.right() - 12.0;
            let cy = tb_rect.center().y;
            let close_r = Rect::from_center_size(Pos2::new(right - 14.0, cy), Vec2::splat(28.0));
            let min_r = Rect::from_center_size(
                Pos2::new(close_r.left() - 18.0, cy),
                Vec2::splat(28.0),
            );
            let drag_rect = Rect::from_min_max(
                tb_rect.min,
                Pos2::new(min_r.left() - 6.0, tb_rect.bottom()),
            );
            let tb_resp = ui.interact(drag_rect, Id::unique("opt_tb_drag"), Sense::drag());
            let min_resp = ui.interact(min_r, Id::unique("opt_min"), Sense::click());
            let close_resp = ui.interact(close_r, Id::unique("opt_close"), Sense::click());

            if tb_resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            paint_chrome_btn(ui.painter(), min_r, &min_resp, glyph::REMOVE, false, &pal);
            paint_chrome_btn(ui.painter(), close_r, &close_resp, glyph::CLOSE, true, &pal);
            if min_resp.clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            if close_resp.clicked() {
                core.lock().unwrap().options_open = false;
                ctx.send_viewport_cmd(ViewportCommand::Close);
                ctx.request_repaint_of(ViewportId::ROOT);
            }

            let body_h = ui.available_height();
            let (compact_now, active_tab, has_any_update) = {
                let g = core.lock().unwrap();
                (g.sidebar_compact, g.options_tab, g.has_pending_update())
            };

            let t_anim = ctx.animate_bool_with_time(Id::unique("nb_opts_sb_anim"), compact_now, 0.16);
            let sb_w = egui::lerp(196.0..=52.0, t_anim);

            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(sb_w, body_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        let sb_rect = ui.available_rect_before_wrap();
                        ui.painter()
                            .rect_filled(sb_rect, CornerRadius::ZERO, pal.surface);
                        ui.painter().line_segment(
                            [sb_rect.right_top(), sb_rect.right_bottom()],
                            Stroke::new(1.0, pal.outline),
                        );
                        ui.set_clip_rect(sb_rect);

                        egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
                            let tog_rect = Rect::from_min_size(
                                ui.cursor().min,
                                Vec2::new((sb_w - 16.0).max(36.0), 32.0),
                            );
                            let tog_resp = ui.allocate_rect(tog_rect, Sense::click());
                            if tog_resp.hovered() {
                                ui.painter().rect_filled(
                                    tog_rect,
                                    CornerRadius::same(8),
                                    pal.surface_elevated,
                                );
                            }
                            paint_glyph(
                                ui.painter(),
                                Pos2::new(tog_rect.left() + 18.0, tog_rect.center().y),
                                if compact_now {
                                    glyph::CHEVRON_RIGHT
                                } else {
                                    glyph::DOCK_TO_LEFT
                                },
                                18.0,
                                pal.on_surface_muted,
                            );
                            if tog_resp.clicked() {
                                let mut g = core.lock().unwrap();
                                g.sidebar_compact = !g.sidebar_compact;
                                g.live.ui_settings.settings_sidebar_compact = g.sidebar_compact;
                                g.persist_ui_settings();
                            }

                            ui.add_space(6.0);

                            let nav_items = [
                                (OptionsTab::Appearance, glyph::PALETTE, "nav_appearance", false),
                                (OptionsTab::Network, glyph::LAN, "nav_network", false),
                                (
                                    OptionsTab::Updates,
                                    glyph::SYSTEM_UPDATE_ALT,
                                    "nav_updates",
                                    has_any_update,
                                ),
                                (OptionsTab::About, glyph::INFO, "nav_about", false),
                            ];

                            for (tab_id, ic, key, show_dot) in nav_items {
                                let lbl = { core.lock().unwrap().t(key).to_string() };
                                let selected = active_tab == tab_id;
                                let btn_rect = Rect::from_min_size(
                                    ui.cursor().min,
                                    Vec2::new((sb_w - 16.0).max(36.0), 36.0),
                                );
                                let resp = ui.allocate_rect(btn_rect, Sense::click());
                                let (bg, border, fg) = if selected {
                                    (pal.accent_soft, pal.accent_border, pal.accent_on_soft)
                                } else if resp.hovered() {
                                    (pal.surface_elevated, Color32::TRANSPARENT, pal.on_surface)
                                } else {
                                    (
                                        Color32::TRANSPARENT,
                                        Color32::TRANSPARENT,
                                        pal.on_surface_muted,
                                    )
                                };
                                ui.painter().rect(
                                    btn_rect,
                                    CornerRadius::same(8),
                                    bg,
                                    Stroke::new(1.0, border),
                                    StrokeKind::Inside,
                                );

                                paint_glyph(
                                    ui.painter(),
                                    Pos2::new(btn_rect.left() + 18.0, btn_rect.center().y),
                                    ic,
                                    18.0,
                                    fg,
                                );

                                let label_alpha = (1.0 - t_anim * 1.4).clamp(0.0, 1.0);
                                if label_alpha > 0.02 {
                                    ui.painter().text(
                                        Pos2::new(btn_rect.left() + 36.0, btn_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        lbl,
                                        semibold(12.5),
                                        fg.gamma_multiply(label_alpha),
                                    );
                                }
                                if show_dot {
                                    let dot_pos = if compact_now {
                                        Pos2::new(btn_rect.right() - 7.0, btn_rect.top() + 7.0)
                                    } else {
                                        Pos2::new(btn_rect.right() - 10.0, btn_rect.center().y)
                                    };
                                    ui.painter().circle_filled(dot_pos, 4.0, pal.accent);
                                }
                                if resp.clicked() {
                                    core.lock().unwrap().options_tab = tab_id;
                                }
                                ui.add_space(4.0);
                            }
                        });
                    },
                );

                let content_w = ui.available_width().max(200.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(content_w, body_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        egui::Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| match active_tab {
                                    OptionsTab::Appearance => {
                                        appearance::render_opt_appearance(core, ui, &pal)
                                    }
                                    OptionsTab::Network => {
                                        network::render_opt_network(core, ui, &pal)
                                    }
                                    OptionsTab::Updates => {
                                        updates::render_opt_updates(core, ui, &pal)
                                    }
                                    OptionsTab::About => about::render_opt_about(core, ui, &pal),
                                });
                        });
                    },
                );
            });

            render_hsv_color_popover(core, ui, &pal);
        });

    let pal_change = {
        let mut g = core.lock().unwrap();
        let pal_after = g.palette();
        if pal_after != pal {
            let dwm_key = (pal_after.is_dark, pal_after.outline);
            g.last_visuals_pal = Some(pal_after);
            g.last_dwm_opts_key = Some(dwm_key);
            g.last_dwm_main_key = Some(dwm_key);
            let main_title = g.app_title.clone();
            Some((pal_after, main_title))
        } else {
            None
        }
    };
    if let Some((pal_after, main_title)) = pal_change {
        apply_egui_visuals(&ctx, &pal_after);
        apply_win32_dwm_to_title(opt_title, &pal_after);
        apply_win32_dwm_to_title(&main_title, &pal_after);
        super::repaint_all_viewports(&ctx);
    }
}
