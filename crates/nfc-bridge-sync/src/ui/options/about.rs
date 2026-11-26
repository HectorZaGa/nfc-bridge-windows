use super::super::state::SharedUiCore;
use super::super::theme::ThemePalette;
use super::super::widgets::{about_row, paint_divider};
use egui::{CornerRadius, Margin, Stroke};
use std::sync::{Arc, Mutex};

pub(crate) fn render_opt_about(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        hdr,
        app_name,
        app_ver,
        mode_lbl,
        mode_val,
        drv_ver_lbl,
        drv_state_lbl,
        drv_state_val,
        drv_online,
        proto_lbl,
        proto_val,
    ) = {
        let g = core.lock().unwrap();
        let u = &g.updates;
        let name = g.app_display_name();
        let ver = format!("v{} ({})", g.live.sync_version, g.live.sync_build_id);
        let dver = match (
            &g.live.snapshot.driver_version,
            &g.live.snapshot.driver_build_id,
        ) {
            (Some(v), Some(b)) => format!("v{} ({})", v, b),
            (Some(v), None) => format!("v{}", v),
            _ => g.t("about_driver_missing").to_string(),
        };
        let dst = g
            .t(if g.live.driver_online {
                "about_driver_active"
            } else {
                "about_driver_stopped"
            })
            .to_string();
        (
            g.t("settings_about_header").to_uppercase(),
            name,
            ver,
            g.t("about_exec_mode_label").to_string(),
            g.t(if u.is_portable {
                "about_exec_portable"
            } else {
                "about_exec_installed"
            })
            .to_string(),
            dver,
            g.t("about_driver_state_label").to_string(),
            dst,
            g.live.driver_online,
            g.t("about_protocol_label").to_string(),
            g.t("about_protocol_val").to_string(),
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
            about_row(ui, pal, &app_name, &app_ver, true, pal.on_surface);
            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);
            about_row(ui, pal, "NFC Bridge Driver", &drv_ver_lbl, true, pal.on_surface);
            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);
            about_row(ui, pal, &mode_lbl, &mode_val, false, pal.on_surface_muted);
            ui.add_space(8.0);
            about_row(
                ui,
                pal,
                &drv_state_lbl,
                &drv_state_val,
                false,
                if drv_online {
                    pal.led_green
                } else {
                    pal.red_disconnect
                },
            );
            ui.add_space(8.0);
            about_row(ui, pal, &proto_lbl, &proto_val, false, pal.on_surface_muted);
        });
}
