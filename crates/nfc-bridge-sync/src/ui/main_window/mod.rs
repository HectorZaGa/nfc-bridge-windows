mod device_dropdown;
pub(crate) mod left_column;
mod modals;
pub(crate) mod panels;

use super::icons::{glyph, paint_glyph, paint_update_down_badge, semibold};
use super::state::SharedUiCore;
use super::widgets::{paint_chrome_btn, paint_pill};
use device_dropdown::render_device_dropdown_overlay;
use egui::{
    Align, CornerRadius, Id, Layout, Margin, Pos2, Rect, Sense, Stroke, Vec2, ViewportCommand,
};
use left_column::render_left_column;
use modals::render_body_modals;
use panels::{render_qr_panel, render_transport_panel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn render_main_viewport(
    core: &Arc<Mutex<SharedUiCore>>,
    ctx: &egui::Context,
    ui: &mut egui::Ui,
) {
    let pal = { core.lock().unwrap().palette() };

    // Auto-renovar enlace live de Relay cada ~75 s si aplica
    {
        let mut g = core.lock().unwrap();
        let need_relay_refresh = g.live.driver_online
            && g.live
                .snapshot
                .relay_url
                .as_deref()
                .is_some_and(|u| !u.is_empty())
            && g.live_relay_expires_at
                .is_none_or(|exp| Instant::now() >= exp);
        if need_relay_refresh {
            g.live_relay_expires_at = Some(Instant::now() + Duration::from_secs(75));
            let ipc = g.ipc.clone();
            let core_c = core.clone();
            let ctx_c = ctx.clone();
            g.rt.spawn(async move {
                if let Ok(qr) = ipc.request_pairing_qr("relay").await {
                    let mut guard = core_c.lock().unwrap();
                    guard.live_relay_link = qr.share_link;
                    guard.live_relay_expires_at = Some(Instant::now() + Duration::from_secs(75));
                }
                ctx_c.request_repaint();
            });
        }
    }

    egui::Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.outline))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // 1) Custom Frameless Titlebar (48 px)
            let (titlebar_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::hover());
            ui.painter()
                .rect_filled(titlebar_rect, CornerRadius::ZERO, pal.surface_card);
            ui.painter().line_segment(
                [titlebar_rect.left_bottom(), titlebar_rect.right_bottom()],
                Stroke::new(1.0, pal.outline),
            );

            let (app_title_text, pc_name, ver_text, has_update, active_panel, qr_method) = {
                let g = core.lock().unwrap();
                let title = g.app_display_name();
                let pc = if g.live.snapshot.pc_name.is_empty() {
                    g.t("pc_windows_default").to_string()
                } else {
                    g.live.snapshot.pc_name.clone()
                };
                let ver = format!("v{} · {}", g.live.sync_version, g.live.sync_build_id);
                (
                    title,
                    pc,
                    ver,
                    g.has_pending_update(),
                    g.active_panel.clone(),
                    g.qr_method.clone(),
                )
            };

            let logo_center = Pos2::new(titlebar_rect.left() + 26.0, titlebar_rect.center().y);
            paint_glyph(
                ui.painter(),
                logo_center,
                glyph::NFC,
                22.0,
                pal.accent,
            );

            let mut x_cur = titlebar_rect.left() + 44.0;
            let title_galley =
                ui.painter()
                    .layout_no_wrap(app_title_text, semibold(16.0), pal.on_surface);
            let title_w = title_galley.size().x;
            ui.painter().galley(
                Pos2::new(x_cur, titlebar_rect.center().y - title_galley.size().y * 0.5),
                title_galley,
                pal.on_surface,
            );
            x_cur += title_w + 8.0;

            x_cur = paint_pill(
                ui,
                Pos2::new(x_cur, titlebar_rect.center().y),
                &pc_name,
                12.0,
                false,
                false,
                &pal,
            );
            x_cur += 8.0;
            let _ = paint_pill(
                ui,
                Pos2::new(x_cur, titlebar_rect.center().y),
                &ver_text,
                10.0,
                true,
                true,
                &pal,
            );

            let right = titlebar_rect.right() - 16.0;
            let cy = titlebar_rect.center().y;
            let close_rect =
                Rect::from_center_size(Pos2::new(right - 16.0, cy), Vec2::splat(32.0));
            let min_rect = Rect::from_center_size(
                Pos2::new(close_rect.left() - 20.0, cy),
                Vec2::splat(32.0),
            );
            let opt_rect =
                Rect::from_center_size(Pos2::new(min_rect.left() - 20.0, cy), Vec2::splat(32.0));

            let drag_rect = Rect::from_min_max(
                titlebar_rect.min,
                Pos2::new(opt_rect.left() - 6.0, titlebar_rect.bottom()),
            );
            let titlebar_resp = ui.interact(drag_rect, Id::unique("tb_drag"), Sense::drag());

            let opt_resp = ui.interact(opt_rect, Id::unique("tb_opt"), Sense::click());
            let min_resp = ui.interact(min_rect, Id::unique("tb_min"), Sense::click());
            let close_resp = ui.interact(close_rect, Id::unique("tb_close"), Sense::click());

            if titlebar_resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }

            paint_chrome_btn(ui.painter(), opt_rect, &opt_resp, glyph::SETTINGS, false, &pal);
            if has_update {
                paint_update_down_badge(ui.painter(), opt_rect, pal.surface_card, pal.accent);
            }
            paint_chrome_btn(ui.painter(), min_rect, &min_resp, glyph::REMOVE, false, &pal);
            paint_chrome_btn(ui.painter(), close_rect, &close_resp, glyph::CLOSE, true, &pal);

            if opt_resp.clicked() {
                core.lock().unwrap().open_options(None);
            }
            if min_resp.clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            if close_resp.clicked() {
                let mut g = core.lock().unwrap();
                g.main_visible = false;
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }

            // 2) Body Area (modales oscurecen solo esta área, sin tapar el titlebar)
            let body_rect = ui.available_rect_before_wrap();
            let mut card_anchor_rect = Rect::NOTHING;
            let mut sel_btn_rect = Rect::NOTHING;
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(body_rect)
                    .layout(Layout::top_down(Align::Min)),
                |ui| {
                    egui::Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                        let avail_w = ui.available_width();
                        let col_gap = 16.0;
                        let left_w = ((avail_w - col_gap) * (1.28 / 2.20)).floor();
                        let right_w = avail_w - col_gap - left_w;
                        let col_h = ui.available_height();

                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                Vec2::new(left_w, col_h),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    (card_anchor_rect, sel_btn_rect) =
                                        render_left_column(core, ui, &pal, &active_panel);
                                },
                            );
                            ui.add_space(col_gap);
                            ui.allocate_ui_with_layout(
                                Vec2::new(right_w, col_h),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    egui::Frame::new()
                                        .fill(pal.surface_card)
                                        .stroke(Stroke::new(1.0, pal.outline))
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(Margin::same(16))
                                        .show(ui, |ui| {
                                            ui.set_min_size(Vec2::new(
                                                right_w - 34.0,
                                                (col_h - 34.0).max(200.0),
                                            ));
                                            if let Some(ref m) = qr_method {
                                                render_qr_panel(core, ui, &pal, m);
                                            } else {
                                                render_transport_panel(
                                                    core,
                                                    ui,
                                                    &pal,
                                                    &active_panel,
                                                );
                                            }
                                        });
                                },
                            );
                        });
                    });

                    render_device_dropdown_overlay(core, ui, &pal, card_anchor_rect, sel_btn_rect);
                    render_body_modals(core, ui, &pal, body_rect);
                },
            );
        });
}
