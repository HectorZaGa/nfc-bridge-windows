use super::super::super::icons::glyph;
use super::super::super::qr::paint_qr_box;
use super::super::super::state::{IssuedQrView, SharedUiCore};
use super::super::super::theme::ThemePalette;
use super::super::super::widgets::{paint_status_badge, secondary_btn, small_ghost_btn};
use egui::{Align, Color32, CornerRadius, FontId, Layout, Margin, RichText, Sense, Stroke, Vec2};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn render_qr_panel(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    method: &str,
) {
    let (
        back_lbl,
        method_lbl,
        loading,
        err,
        qr_opt,
        copy_temp_lbl,
        link_copied_lbl,
        copied_temp,
        refresh_lbl,
        generating_lbl,
    ) = {
        let g = core.lock().unwrap();
        let m_lbl = match method {
            "tcp" => g.t("qr_method_tcp"),
            "bluetooth" => g.t("qr_method_bluetooth"),
            "wifi_direct" => g.t("qr_method_wifi_direct"),
            _ => g.t("qr_method_relay"),
        };
        let now = Instant::now();
        (
            g.t("qr_back").to_string(),
            m_lbl.to_string(),
            g.qr_loading,
            g.qr_error.clone(),
            g.qr_data.clone(),
            g.t("qr_copy_temp_link").to_string(),
            g.t("qr_link_copied").to_string(),
            g.copied_key
                .as_ref()
                .is_some_and(|(k, exp)| k == "qr_share" && now < *exp),
            g.t("qr_refresh").to_string(),
            g.t("qr_generating").to_string(),
        )
    };

    ui.horizontal(|ui| {
        if small_ghost_btn(ui, pal, glyph::ARROW_BACK, &back_lbl) {
            let mut g = core.lock().unwrap();
            g.qr_method = None;
            g.qr_error = None;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            paint_status_badge(ui, &method_lbl, true, pal);
        });
    });

    let has_share = qr_opt.as_ref().is_some_and(|q| q.share_link.is_some());
    let bottom_block_h = if qr_opt.is_some() {
        if has_share { 112.0 } else { 76.0 }
    } else {
        36.0
    };
    let mid_space = (ui.available_height() - bottom_block_h - 192.0).max(16.0);
    ui.add_space((mid_space * 0.5).clamp(6.0, 22.0));

    ui.vertical_centered(|ui| {
        let (qr_rect, _) = ui.allocate_exact_size(Vec2::splat(192.0), Sense::hover());
        if loading {
            ui.painter()
                .rect_filled(qr_rect, CornerRadius::same(12), Color32::WHITE);
            ui.painter().text(
                qr_rect.center(),
                egui::Align2::CENTER_CENTER,
                generating_lbl,
                FontId::proportional(12.0),
                Color32::from_rgb(18, 18, 18),
            );
        } else if let Some(ref e) = err {
            ui.painter()
                .rect_filled(qr_rect, CornerRadius::same(12), Color32::WHITE);
            ui.painter().text(
                qr_rect.center(),
                egui::Align2::CENTER_CENTER,
                e,
                FontId::proportional(11.0),
                Color32::from_rgb(127, 41, 41),
            );
        } else if let Some(ref qr) = qr_opt {
            if let Err(e) = paint_qr_box(ui.painter(), qr_rect, &qr.uri, &qr.method, pal.accent) {
                ui.painter().text(
                    qr_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    e,
                    FontId::proportional(11.0),
                    pal.red_disconnect,
                );
            }
        }
    });

    let rem_below_qr = ui.available_height();
    if rem_below_qr > bottom_block_h + 6.0 {
        ui.add_space(((rem_below_qr - bottom_block_h) * 0.5).clamp(6.0, 18.0));
    } else {
        ui.add_space(6.0);
    }

    if let Some(ref qr) = qr_opt {
        let secs_left = qr
            .expires_at
            .saturating_duration_since(Instant::now())
            .as_secs();
        let countdown = format!("{}:{:02}", secs_left / 60, secs_left % 60);

        egui::Frame::new()
            .fill(pal.surface_elevated)
            .stroke(Stroke::new(1.0, pal.outline))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&qr.pair_token)
                            .monospace()
                            .size(12.0)
                            .color(pal.on_surface),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(countdown)
                                .monospace()
                                .size(11.0)
                                .color(pal.accent),
                        );
                    });
                });
            });

        if let Some(ref share) = qr.share_link {
            ui.add_space(4.0);
            let btn_lbl = if copied_temp {
                &link_copied_lbl
            } else {
                &copy_temp_lbl
            };
            if secondary_btn(ui, pal, glyph::CONTENT_COPY, btn_lbl) {
                ui.ctx().copy_text(share.clone());
                core.lock().unwrap().copied_key = Some((
                    "qr_share".to_string(),
                    Instant::now() + Duration::from_secs(2),
                ));
            }
        }
    }

    let avail_h = ui.available_height();
    if avail_h > 36.0 {
        ui.add_space(avail_h - 36.0);
    }
    if secondary_btn(ui, pal, glyph::REFRESH, &refresh_lbl) {
        trigger_qr_request(core, ui.ctx(), method);
    }
}

pub(crate) fn trigger_qr_request(
    core: &Arc<Mutex<SharedUiCore>>,
    ctx: &egui::Context,
    method: &str,
) {
    let m = method.to_string();
    let (ipc, rt) = {
        let mut g = core.lock().unwrap();
        g.qr_method = Some(m.clone());
        g.qr_loading = true;
        g.qr_error = None;
        (g.ipc.clone(), g.rt.clone())
    };
    let core_c = core.clone();
    let ctx_c = ctx.clone();
    rt.spawn(async move {
        match ipc.request_pairing_qr(&m).await {
            Ok(data) => {
                let mut g = core_c.lock().unwrap();
                g.qr_loading = false;
                g.qr_data = Some(IssuedQrView {
                    method: data.method,
                    pair_token: data.pair_token,
                    expires_at: Instant::now() + Duration::from_secs(data.expires_in_secs.max(10)),
                    uri: data.uri,
                    share_link: data.share_link,
                });
            }
            Err(e) => {
                let mut g = core_c.lock().unwrap();
                g.qr_loading = false;
                g.qr_error = Some(e.to_string());
            }
        }
        ctx_c.request_repaint();
    });
}
