use super::super::icons::semibold;
use super::super::state::SharedUiCore;
use super::super::theme::ThemePalette;
use super::super::widgets::{
    paint_divider, small_accent_btn, styled_singleline_input, switch_pill_btn,
};
use crate::ipc::IpcCommand;
use egui::{Align, CornerRadius, FontId, Layout, Margin, Pos2, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn render_opt_network(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        hdr,
        auto_title,
        auto_desc,
        ip_lbl,
        ip_ph,
        port_lbl,
        reset_lbl,
        save_lbl,
        auto_on,
        mut ip_buf,
        mut port_buf,
        fb_opt,
    ) = {
        let g = core.lock().unwrap();
        let now = Instant::now();
        let auto_state = if g.auto_mobile_dirty_until.is_some_and(|t| now < t) {
            g.auto_mobile_optimistic
        } else {
            g.live.snapshot.auto_accept_mobile_switch
        };
        let fb = g.net_feedback.as_ref().and_then(|(s, ok, exp)| {
            if now < *exp {
                Some((s.clone(), *ok))
            } else {
                None
            }
        });
        (
            g.t("settings_network_header").to_uppercase(),
            g.t("settings_auto_mobile_title").to_string(),
            g.t("settings_auto_mobile_desc").to_string(),
            g.t("settings_tcp_ip_label").to_string(),
            g.t("settings_tcp_ip_placeholder").to_string(),
            g.t("settings_tcp_port_label").to_string(),
            g.t("settings_reset_default").to_string(),
            g.t("settings_save_network").to_string(),
            auto_state,
            g.net_ip_buf.clone(),
            g.net_port_buf.clone(),
            fb,
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

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(auto_title)
                            .font(semibold(14.0))
                            .color(pal.on_surface),
                    );
                    ui.label(
                        RichText::new(auto_desc)
                            .size(11.0)
                            .color(pal.on_surface_muted),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let st_lbl = {
                        core.lock()
                            .unwrap()
                            .t(if auto_on { "state_on" } else { "state_off" })
                            .to_string()
                    };
                    if switch_pill_btn(ui, pal, &st_lbl, auto_on) {
                        let mut g = core.lock().unwrap();
                        g.auto_mobile_optimistic = !auto_on;
                        g.auto_mobile_dirty_until =
                            Some(Instant::now() + Duration::from_millis(1600));
                        g.send_ipc(IpcCommand::SetAutoAcceptMobileSwitch { enabled: !auto_on });
                    }
                });
            });

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            let mut ip_has_focus = false;
            let mut port_has_focus = false;
            ui.horizontal(|ui| {
                let total_w = ui.available_width();
                let ip_w = ((total_w - 8.0) * (1.4 / 2.3)).floor();
                let port_w = total_w - 8.0 - ip_w;

                ui.allocate_ui_with_layout(
                    Vec2::new(ip_w, 52.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.label(RichText::new(ip_lbl).size(11.0).color(pal.on_surface_muted));
                        ui.add_space(4.0);
                        let r = styled_singleline_input(ui, pal, &mut ip_buf, ip_w, 32.0, &ip_ph);
                        ip_has_focus = r.has_focus();
                    },
                );
                ui.add_space(8.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(port_w, 52.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.label(RichText::new(port_lbl).size(11.0).color(pal.on_surface_muted));
                        ui.add_space(4.0);
                        let r =
                            styled_singleline_input(ui, pal, &mut port_buf, port_w, 32.0, "45362");
                        port_has_focus = r.has_focus();
                        port_buf.retain(|c| c.is_ascii_digit());
                        if port_buf.len() > 5 {
                            port_buf.truncate(5);
                        }
                    },
                );
            });

            {
                let mut g = core.lock().unwrap();
                let dirty = g.net_dirty_until.is_some_and(|t| Instant::now() < t);
                if ip_has_focus || port_has_focus || dirty {
                    g.net_ip_buf = ip_buf.clone();
                    g.net_port_buf = port_buf.clone();
                } else {
                    g.net_ip_buf = g.live.snapshot.tcp_ip_override.clone().unwrap_or_default();
                    g.net_port_buf = g.live.snapshot.tcp_port.to_string();
                }
            }

            ui.add_space(10.0);
            let row_w = ui.available_width();
            ui.allocate_ui_with_layout(
                Vec2::new(row_w, 32.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    if let Some((fb_txt, ok)) = fb_opt {
                        ui.label(
                            RichText::new(fb_txt)
                                .size(11.0)
                                .color(if ok { pal.accent } else { pal.red_disconnect }),
                        );
                    }
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 32.0),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            if small_accent_btn(ui, pal, &save_lbl, 96.0, 32.0) {
                                let mut g = core.lock().unwrap();
                                let parsed_port =
                                    port_buf.parse::<u16>().ok().filter(|&p| p >= 1024);
                                if let Some(p) = parsed_port {
                                    let ip_opt = if ip_buf.trim().is_empty() {
                                        None
                                    } else {
                                        Some(ip_buf.trim().to_string())
                                    };
                                    g.net_ip_buf = ip_opt.clone().unwrap_or_default();
                                    g.net_port_buf = p.to_string();
                                    g.net_dirty_until =
                                        Some(Instant::now() + Duration::from_millis(2000));
                                    g.net_feedback = Some((
                                        g.t("settings_net_saved").to_string(),
                                        true,
                                        Instant::now() + Duration::from_secs(4),
                                    ));
                                    g.send_ipc(IpcCommand::SetTcpNetworkConfig {
                                        tcp_ip: ip_opt,
                                        tcp_port: Some(p),
                                        reset: false,
                                    });
                                } else {
                                    let msg = g.t("settings_net_invalid_port").to_string();
                                    g.net_feedback =
                                        Some((msg, false, Instant::now() + Duration::from_secs(4)));
                                }
                            }
                            ui.add_space(8.0);
                            let rst_gal = ui.painter().layout_no_wrap(
                                reset_lbl.clone(),
                                FontId::proportional(11.0),
                                egui::Color32::PLACEHOLDER,
                            );
                            let rst_w = (rst_gal.size().x + 24.0).max(120.0);
                            let (rr, rresp) =
                                ui.allocate_exact_size(Vec2::new(rst_w, 32.0), Sense::click());
                            ui.painter().rect(
                                rr,
                                CornerRadius::same(8),
                                pal.surface,
                                Stroke::new(
                                    1.0,
                                    if rresp.hovered() {
                                        pal.accent
                                    } else {
                                        pal.outline
                                    },
                                ),
                                StrokeKind::Inside,
                            );
                            ui.painter().galley(
                                Pos2::new(
                                    rr.center().x - rst_gal.size().x * 0.5,
                                    rr.center().y - rst_gal.size().y * 0.5,
                                ),
                                rst_gal,
                                if rresp.hovered() {
                                    pal.on_surface
                                } else {
                                    pal.on_surface_muted
                                },
                            );
                            if rresp.clicked() {
                                let mut g = core.lock().unwrap();
                                g.net_ip_buf.clear();
                                g.net_port_buf = "45362".to_string();
                                g.net_dirty_until =
                                    Some(Instant::now() + Duration::from_millis(2000));
                                g.net_feedback = Some((
                                    g.t("settings_net_reset_done").to_string(),
                                    true,
                                    Instant::now() + Duration::from_secs(4),
                                ));
                                g.send_ipc(IpcCommand::SetTcpNetworkConfig {
                                    tcp_ip: None,
                                    tcp_port: None,
                                    reset: true,
                                });
                            }
                        },
                    );
                },
            );
        });
}
