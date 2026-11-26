use super::super::super::icons::glyph;
use super::super::super::state::SharedUiCore;
use super::super::super::theme::ThemePalette;
use super::super::super::widgets::{info_kv_row, primary_cta_btn, toggle_state_btn};
use super::qr::trigger_qr_request;
use super::{panel_header, panel_push_footer};
use crate::ipc::IpcCommand;
use egui::RichText;
use std::sync::{Arc, Mutex};

pub(crate) fn render_panel_bt(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        title,
        status_txt,
        bt_enabled,
        bt_on_flag,
        adapter_val,
        mac_val,
        adapter_lbl,
        mac_lbl,
        body_desc,
        qr_lbl,
        toggle_lbl,
        state_lbl,
    ) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let adapter = snap.bt_name.clone().unwrap_or_else(|| {
            if snap.pc_name.is_empty() {
                "Windows BT".to_string()
            } else {
                snap.pc_name.clone()
            }
        });
        let mac = snap
            .bt_mac
            .clone()
            .unwrap_or_else(|| g.t("panel_bt_unavailable").to_string());
        let st_txt = if !snap.bt_available {
            g.t("panel_bt_no_radio")
        } else if !snap.bt_enabled {
            g.t("state_off")
        } else {
            g.t("panel_bt_status_active")
        };
        (
            g.t("method_bt_title").to_string(),
            st_txt.to_string(),
            snap.bt_enabled && snap.bt_available,
            snap.bt_enabled,
            adapter,
            mac,
            g.t("panel_bt_adapter_label").to_string(),
            g.t("panel_bt_mac_label").to_string(),
            g.t("panel_bt_desc_qr").to_string(),
            g.t("btn_generate_qr").to_string(),
            g.t(if snap.bt_enabled {
                "panel_bt_toggle_disable"
            } else {
                "panel_bt_toggle_enable"
            })
            .to_string(),
            g.t(if snap.bt_enabled {
                "panel_bt_status_active"
            } else {
                "state_off"
            })
            .to_string(),
        )
    };

    panel_header(ui, pal, &title, &status_txt, bt_enabled);

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(120.0);
    ui.add_space(((body_h - 114.0) * 0.5).max(12.0));

    info_kv_row(ui, pal, &adapter_lbl, &adapter_val, true);
    ui.add_space(8.0);
    info_kv_row(ui, pal, &mac_lbl, &mac_val, false);
    ui.add_space(10.0);
    ui.label(
        RichText::new(body_desc)
            .size(11.0)
            .color(pal.on_surface_muted),
    );

    panel_push_footer(ui, footer_h);

    if toggle_state_btn(
        ui,
        pal,
        if bt_on_flag {
            glyph::BLUETOOTH
        } else {
            glyph::BLUETOOTH_DISABLED
        },
        &toggle_lbl,
        &state_lbl,
        bt_on_flag,
    ) {
        core.lock()
            .unwrap()
            .send_ipc(IpcCommand::SetBluetoothEnabled {
                enabled: !bt_on_flag,
            });
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::QR_CODE, &qr_lbl, true) {
        trigger_qr_request(core, ui.ctx(), "bluetooth");
    }
}
