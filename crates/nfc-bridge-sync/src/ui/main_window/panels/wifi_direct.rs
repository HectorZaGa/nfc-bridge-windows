use super::super::super::icons::glyph;
use super::super::super::state::SharedUiCore;
use super::super::super::theme::ThemePalette;
use super::super::super::widgets::{
    info_copy_tile_w_col, info_kv_row, primary_cta_btn, toggle_state_btn,
};
use super::qr::trigger_qr_request;
use super::{panel_header, panel_push_footer};
use crate::ipc::IpcCommand;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn render_panel_wfd(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        title,
        status_txt,
        wfd_active,
        wfd_enabled,
        wfd_state_lbl,
        ssid_val,
        pass_val,
        ip_val,
        ssid_lbl,
        pass_lbl,
        ip_lbl,
        toggle_lbl,
        qr_lbl,
        copied_ssid,
        copied_pass,
    ) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let st_txt = if !snap.wfd_enabled {
            g.t("panel_wfd_status_inactive")
        } else if snap.wfd_available {
            g.t("panel_wfd_status_active")
        } else {
            g.t("panel_wfd_status_no_wifi")
        };
        let state_r = if snap.wfd_enabled {
            g.t(if snap.wfd_available {
                "panel_wfd_state_open"
            } else {
                "panel_wfd_state_waiting"
            })
        } else {
            g.t("panel_wfd_state_closed")
        };
        let ssid = snap.wfd_ssid.clone().unwrap_or_else(|| {
            g.t(if snap.wfd_enabled {
                "panel_wfd_ssid_activating"
            } else {
                "panel_wfd_ssid_hint"
            })
            .to_string()
        });
        let pass = snap.wfd_passphrase.clone().unwrap_or_else(|| "--".to_string());
        let ip = format!(
            "{}:{}",
            snap.wfd_ip.as_deref().unwrap_or("192.168.137.1"),
            snap.tcp_port
        );
        let now = Instant::now();
        (
            g.t("method_wfd_title").to_string(),
            st_txt.to_string(),
            snap.wfd_enabled && snap.wfd_available,
            snap.wfd_enabled,
            state_r.to_string(),
            ssid,
            pass,
            ip,
            g.t("panel_wfd_ssid_label").to_string(),
            g.t("panel_wfd_pass_label").to_string(),
            g.t("panel_wfd_ip_label").to_string(),
            g.t(if snap.wfd_enabled {
                "panel_wfd_close"
            } else {
                "panel_wfd_open"
            })
            .to_string(),
            g.t("btn_generate_qr").to_string(),
            g.copied_key
                .as_ref()
                .is_some_and(|(k, exp)| k == "wfd_ssid" && now < *exp),
            g.copied_key
                .as_ref()
                .is_some_and(|(k, exp)| k == "wfd_pass" && now < *exp),
        )
    };

    panel_header(ui, pal, &title, &status_txt, wfd_active);

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(140.0);
    ui.add_space(((body_h - 152.0) * 0.5).max(10.0));

    if info_copy_tile_w_col(
        ui,
        pal,
        ui.available_width(),
        &ssid_lbl,
        &ssid_val,
        copied_ssid,
        pal.on_surface,
    ) {
        ui.ctx().copy_text(ssid_val.clone());
        core.lock().unwrap().copied_key = Some((
            "wfd_ssid".to_string(),
            Instant::now() + Duration::from_secs(2),
        ));
    }
    ui.add_space(8.0);
    if info_copy_tile_w_col(
        ui,
        pal,
        ui.available_width(),
        &pass_lbl,
        &pass_val,
        copied_pass,
        pal.accent,
    ) {
        ui.ctx().copy_text(pass_val.clone());
        core.lock().unwrap().copied_key = Some((
            "wfd_pass".to_string(),
            Instant::now() + Duration::from_secs(2),
        ));
    }
    ui.add_space(8.0);
    info_kv_row(ui, pal, &ip_lbl, &ip_val, false);

    panel_push_footer(ui, footer_h);

    if toggle_state_btn(
        ui,
        pal,
        glyph::WIFI_TETHERING,
        &toggle_lbl,
        &wfd_state_lbl,
        wfd_enabled,
    ) {
        core.lock()
            .unwrap()
            .send_ipc(IpcCommand::SetWifiDirectEnabled {
                enabled: !wfd_enabled,
            });
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::QR_CODE, &qr_lbl, true) {
        trigger_qr_request(core, ui.ctx(), "wifi_direct");
    }
}
