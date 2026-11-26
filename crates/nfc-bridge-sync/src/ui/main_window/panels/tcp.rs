use super::super::super::icons::glyph;
use super::super::super::state::SharedUiCore;
use super::super::super::theme::ThemePalette;
use super::super::super::widgets::{info_copy_tile_w, primary_cta_btn, toggle_state_btn};
use super::qr::trigger_qr_request;
use super::{panel_header, panel_push_footer};
use crate::ipc::IpcCommand;
use egui::RichText;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn render_panel_tcp(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        title,
        desc,
        status_txt,
        tcp_enabled,
        ip_val,
        port_val,
        ip_lbl,
        port_lbl,
        qr_lbl,
        toggle_lbl,
        state_lbl,
        copied_ip,
        copied_port,
    ) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let ip = snap.tcp_ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        let now = Instant::now();
        (
            g.t("method_tcp_title").to_string(),
            g.t("panel_tcp_desc").to_string(),
            g.t(if snap.tcp_enabled {
                "panel_tcp_status_listening"
            } else {
                "state_off"
            })
            .to_string(),
            snap.tcp_enabled,
            ip,
            snap.tcp_port.to_string(),
            g.t("panel_tcp_ip_label").to_string(),
            g.t("panel_tcp_port_label").to_string(),
            g.t("btn_generate_qr").to_string(),
            g.t(if snap.tcp_enabled {
                "panel_tcp_toggle_disable"
            } else {
                "panel_tcp_toggle_enable"
            })
            .to_string(),
            g.t(if snap.tcp_enabled {
                "panel_bt_status_active"
            } else {
                "state_off"
            })
            .to_string(),
            g.copied_key
                .as_ref()
                .is_some_and(|(k, exp)| k == "tcp_ip" && now < *exp),
            g.copied_key
                .as_ref()
                .is_some_and(|(k, exp)| k == "tcp_port" && now < *exp),
        )
    };

    panel_header(ui, pal, &title, &status_txt, tcp_enabled);

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(110.0);
    ui.add_space(((body_h - 86.0) * 0.5).max(12.0));

    let full_w = ui.available_width();
    let gap = 4.0;
    let w_left = ((full_w - gap) * (1.4 / 2.2)).floor();
    let w_right = full_w - gap - w_left;
    ui.horizontal(|ui| {
        if info_copy_tile_w(ui, pal, w_left, &ip_lbl, &ip_val, copied_ip) {
            ui.ctx().copy_text(ip_val.clone());
            core.lock().unwrap().copied_key =
                Some(("tcp_ip".to_string(), Instant::now() + Duration::from_secs(2)));
        }
        ui.add_space(gap);
        if info_copy_tile_w(ui, pal, w_right, &port_lbl, &port_val, copied_port) {
            ui.ctx().copy_text(port_val.clone());
            core.lock().unwrap().copied_key = Some((
                "tcp_port".to_string(),
                Instant::now() + Duration::from_secs(2),
            ));
        }
    });

    ui.add_space(12.0);
    ui.label(RichText::new(desc).size(11.0).color(pal.on_surface_muted));

    panel_push_footer(ui, footer_h);

    if toggle_state_btn(
        ui,
        pal,
        if tcp_enabled {
            glyph::WIFI
        } else {
            glyph::WIFI_OFF
        },
        &toggle_lbl,
        &state_lbl,
        tcp_enabled,
    ) {
        core.lock().unwrap().send_ipc(IpcCommand::SetTcpEnabled {
            enabled: !tcp_enabled,
        });
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::QR_CODE, &qr_lbl, true) {
        trigger_qr_request(core, ui.ctx(), "tcp");
    }
}
