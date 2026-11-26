use super::super::super::icons::glyph;
use super::super::super::state::SharedUiCore;
use super::super::super::theme::ThemePalette;
use super::super::super::widgets::{
    info_kv_row, primary_cta_btn, secondary_btn_accent_icon, small_secondary_btn,
    styled_singleline_input, truncate_text_to_width,
};
use super::qr::trigger_qr_request;
use super::{panel_header, panel_push_footer};
use crate::ipc::IpcCommand;
use egui::{CornerRadius, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn render_panel_relay(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        title,
        status_txt,
        status_on,
        url_lbl,
        save_lbl,
        ch_lbl,
        ch_val,
        link_lbl,
        live_link,
        renew_lbl,
        qr_lbl,
        relay_desc,
        copy_lbl,
        copied_txt,
        copied_link,
        has_url,
    ) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let has_u = snap.relay_url.as_deref().is_some_and(|u| !u.is_empty());
        let st_txt = if snap.relay_connected {
            g.t("panel_relay_status_connected")
        } else if has_u {
            g.t("panel_relay_status_reconnecting")
        } else {
            g.t("panel_relay_status_inactive")
        };
        let now = Instant::now();
        (
            g.t("method_relay_title").to_string(),
            st_txt.to_string(),
            snap.relay_connected,
            g.t("panel_relay_url_label").to_string(),
            g.t("btn_save").to_string(),
            g.t("panel_relay_channel_label").to_string(),
            snap.relay_channel.clone(),
            g.t("panel_relay_live_link_label").to_string(),
            g.live_relay_link.clone(),
            g.t("panel_relay_refresh_link").to_string(),
            g.t("btn_generate_qr").to_string(),
            g.t("panel_relay_desc").to_string(),
            g.t("btn_copy").to_string(),
            g.t("btn_copied").to_string(),
            g.copied_key
                .as_ref()
                .is_some_and(|(k, exp)| k == "relay_link" && now < *exp),
            has_u,
        )
    };

    panel_header(ui, pal, &title, &status_txt, status_on);

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(150.0);
    let inner_h = if has_url && live_link.is_some() {
        196.0
    } else {
        142.0
    };
    ui.add_space(((body_h - inner_h) * 0.5).max(10.0));

    ui.label(
        RichText::new(url_lbl)
            .size(11.0)
            .color(pal.on_surface_muted),
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let mut g = core.lock().unwrap();
        let btn_w = 74.0;
        let gap = 6.0;
        let w = (ui.available_width() - btn_w - gap).max(120.0);
        styled_singleline_input(
            ui,
            pal,
            &mut g.relay_url_buf,
            w,
            36.0,
            "https://nfc-bridge.hectorzaga96.workers.dev",
        );
        ui.add_space(gap);
        if small_secondary_btn(ui, pal, &save_lbl, btn_w, 36.0) {
            let trimmed = g.relay_url_buf.trim().to_string();
            let url_opt = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
            g.send_ipc(IpcCommand::SetRelayUrl { url: url_opt });
            g.live_relay_expires_at = None;
        }
    });

    ui.add_space(8.0);
    info_kv_row(
        ui,
        pal,
        &ch_lbl,
        if ch_val.is_empty() { "nb-..." } else { &ch_val },
        false,
    );

    if let Some(ref ll) = live_link {
        ui.add_space(8.0);
        let secs_left = core
            .lock()
            .unwrap()
            .live_relay_expires_at
            .map(|exp| exp.saturating_duration_since(Instant::now()).as_secs())
            .unwrap_or(90);
        let countdown = format!("{}:{:02}", secs_left / 60, secs_left % 60);
        let box_w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(Vec2::new(box_w, 48.0), Sense::hover());
        ui.painter().rect(
            r,
            CornerRadius::same(8),
            pal.surface_elevated,
            Stroke::new(1.0, pal.outline),
            StrokeKind::Inside,
        );
        ui.painter().text(
            Pos2::new(r.left() + 12.0, r.top() + 8.0),
            egui::Align2::LEFT_TOP,
            &link_lbl,
            FontId::proportional(11.0),
            pal.on_surface_muted,
        );
        ui.painter().text(
            Pos2::new(r.right() - 12.0, r.top() + 8.0),
            egui::Align2::RIGHT_TOP,
            countdown,
            FontId::monospace(11.0),
            pal.accent,
        );

        let copy_txt = if copied_link {
            &copied_txt
        } else {
            &copy_lbl
        };
        let copy_gal =
            ui.painter()
                .layout_no_wrap(copy_txt.clone(), FontId::proportional(11.0), pal.accent);
        let copy_rect = Rect::from_min_size(
            Pos2::new(r.right() - 12.0 - copy_gal.size().x, r.top() + 25.0),
            Vec2::new(copy_gal.size().x + 4.0, 16.0),
        );
        let copy_resp = ui.interact(
            copy_rect,
            Id::unique("relay_copy_link_btn"),
            Sense::click(),
        );
        let max_url_w = (copy_rect.left() - (r.left() + 12.0) - 10.0).max(60.0);
        let trunc_url =
            truncate_text_to_width(ui.painter(), ll, FontId::monospace(11.0), max_url_w);
        ui.painter().text(
            Pos2::new(r.left() + 12.0, r.top() + 26.0),
            egui::Align2::LEFT_TOP,
            trunc_url,
            FontId::monospace(11.0),
            pal.on_surface,
        );
        ui.painter().galley(
            Pos2::new(copy_rect.left(), r.top() + 26.0),
            copy_gal,
            pal.accent,
        );
        if copy_resp.hovered() {
            ui.painter().line_segment(
                [
                    Pos2::new(copy_rect.left(), r.top() + 39.0),
                    Pos2::new(copy_rect.right() - 4.0, r.top() + 39.0),
                ],
                Stroke::new(1.0, pal.accent),
            );
        }
        if copy_resp.clicked() {
            ui.ctx().copy_text(ll.clone());
            core.lock().unwrap().copied_key = Some((
                "relay_link".to_string(),
                Instant::now() + Duration::from_secs(2),
            ));
        }
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new(relay_desc)
            .size(11.0)
            .color(pal.on_surface_muted),
    );

    panel_push_footer(ui, footer_h);

    if secondary_btn_accent_icon(ui, pal, glyph::REFRESH, &renew_lbl) {
        core.lock().unwrap().live_relay_expires_at = None;
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::QR_CODE, &qr_lbl, true) {
        trigger_qr_request(core, ui.ctx(), "relay");
    }
}
