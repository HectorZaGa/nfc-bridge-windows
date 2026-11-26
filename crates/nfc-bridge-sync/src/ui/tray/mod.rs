pub(crate) mod icon;

use super::icons::{glyph, paint_glyph};
use super::main_window::panels::trigger_qr_request;
use super::state::SharedUiCore;
use super::theme::{apply_win32_dwm_to_title, ThemePalette};
use super::widgets::{paint_divider, paint_inline_capsule};
use crate::ipc::IpcCommand;
use egui::{
    Align, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) fn estimate_tray_height(g: &SharedUiCore) -> f32 {
    let base = 368.0_f32;
    if g.tray_devices_expanded {
        let count = g.live.snapshot.trusted_devices.len().clamp(1, 4) as f32;
        base + count * 30.0 + 6.0
    } else {
        base
    }
}

pub(crate) fn render_tray_viewport(
    core: &Arc<Mutex<SharedUiCore>>,
    tray_title: &str,
    ui: &mut egui::Ui,
) {
    let ctx = ui.ctx().clone();
    let focused = ctx.input(|i| i.viewport().focused);
    {
        let mut g = core.lock().unwrap();
        if focused == Some(false)
            && g.tray_opened_at
                .is_some_and(|t| t.elapsed() > Duration::from_millis(250))
        {
            g.tray_open = false;
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            g.tray_open = false;
            return;
        }
    }

    let (
        pal,
        app_name,
        pc_badge,
        led_color,
        status_txt,
        open_lbl,
        switch_lbl,
        devices,
        active_id,
        selected_id,
        dev_expanded,
        qr_lbl,
        paused,
        pause_lbl,
        usb_prio,
        usb_lbl,
        opt_lbl,
        connected,
        disc_lbl,
        exit_lbl,
    ) = {
        let mut g = core.lock().unwrap();
        let pal = g.palette();
        let dwm_key = (pal.is_dark, pal.outline);
        if g.last_dwm_tray_key != Some(dwm_key) {
            g.last_dwm_tray_key = Some(dwm_key);
            apply_win32_dwm_to_title(tray_title, &pal);
        }
        let st = &g.live;
        let snap = &st.snapshot;
        let (l_c, s_txt) = if !st.driver_online {
            (
                pal.red_disconnect,
                g.t("device_driver_stopped").to_string(),
            )
        } else if st.paused {
            (pal.amber_pause, g.t("device_reader_paused").to_string())
        } else if st.connected {
            let tr = match st.transport.as_deref() {
                Some("usb") => "USB",
                Some("bluetooth") => "BT",
                Some("relay") => "Link",
                Some("wifi_direct") => "WFD",
                _ => "TCP",
            };
            (
                pal.led_green,
                format!(
                    "{} • {} {} ms",
                    st.device_name.as_deref().unwrap_or("Mobile"),
                    tr,
                    st.latency_ms
                ),
            )
        } else {
            (
                pal.on_surface_muted,
                g.t("device_disconnected_short").to_string(),
            )
        };
        (
            pal,
            g.app_display_name(),
            g.t("tray_pc_badge").to_string(),
            l_c,
            s_txt,
            g.t("tray_open_main").to_string(),
            g.t("tray_switch_device").to_string(),
            snap.trusted_devices.clone(),
            st.active_device_id.clone(),
            st.selected_device_id.clone(),
            g.tray_devices_expanded,
            g.t("tray_show_qr").to_string(),
            st.paused,
            g.t(if st.paused {
                "tray_resume_reader"
            } else {
                "tray_pause_reader"
            })
            .to_string(),
            snap.usb_auto_connect,
            g.t("tray_usb_priority").to_string(),
            g.t("tray_options").to_string(),
            st.connected,
            g.t("tray_disconnect").to_string(),
            g.t("tray_exit").to_string(),
        )
    };

    egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

            egui::Frame::new()
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        ui.painter().rect(
                            ic_r,
                            CornerRadius::same(7),
                            pal.accent_soft,
                            Stroke::new(1.0, pal.accent_border),
                            StrokeKind::Inside,
                        );
                        paint_glyph(
                            ui.painter(),
                            ic_r.center(),
                            glyph::CONTACTLESS,
                            16.0,
                            pal.accent,
                        );
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(app_name)
                                    .size(13.0)
                                    .strong()
                                    .color(pal.on_surface),
                            );
                            ui.horizontal(|ui| {
                                let (lr, _) =
                                    ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                                ui.painter().circle_filled(lr.center(), 3.2, led_color);
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(status_txt)
                                        .size(11.0)
                                        .color(pal.on_surface_muted),
                                );
                            });
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            paint_inline_capsule(ui, &pc_badge, true, &pal);
                        });
                    });
                });

            paint_divider(ui, &pal);

            if tray_menu_item(
                ui,
                &pal,
                glyph::OPEN_IN_NEW,
                pal.accent,
                &open_lbl,
                pal.on_surface,
                true,
                None,
                false,
            ) {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.main_visible = true;
                g.main_want_focus = true;
            }

            let chev_ic = if dev_expanded {
                glyph::EXPAND_MORE
            } else {
                glyph::CHEVRON_RIGHT
            };
            if tray_menu_item(
                ui,
                &pal,
                glyph::SMARTPHONE,
                pal.on_surface_muted,
                &switch_lbl,
                pal.on_surface,
                false,
                Some((chev_ic, pal.on_surface_muted)),
                false,
            ) {
                let mut g = core.lock().unwrap();
                g.tray_devices_expanded = !g.tray_devices_expanded;
            }
            if dev_expanded {
                for dev in devices.iter().take(4) {
                    let is_act = Some(&dev.device_id) == active_id.as_ref()
                        || Some(&dev.device_id) == selected_id.as_ref();
                    let (r, resp) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
                    let inner = Rect::from_min_max(Pos2::new(r.left() + 26.0, r.top()), r.max);
                    if is_act || resp.hovered() {
                        ui.painter()
                            .rect_filled(inner, CornerRadius::same(6), pal.surface_elevated);
                    }
                    ui.painter().text(
                        Pos2::new(inner.left() + 8.0, inner.center().y),
                        egui::Align2::LEFT_CENTER,
                        &dev.device_name,
                        FontId::proportional(11.5),
                        if is_act {
                            pal.on_surface
                        } else {
                            pal.on_surface_muted
                        },
                    );
                    if is_act {
                        paint_glyph(
                            ui.painter(),
                            Pos2::new(inner.right() - 12.0, inner.center().y),
                            glyph::CHECK,
                            14.0,
                            pal.led_green,
                        );
                    }
                    if resp.clicked() {
                        let id = dev.device_id.clone();
                        let g = core.lock().unwrap();
                        let ipc = g.ipc.clone();
                        g.rt.spawn(async move {
                            ipc.select_saved_profile(&id).await;
                            let _ = ipc
                                .send_command(IpcCommand::SelectTrustedDevice { device_id: id })
                                .await;
                        });
                    }
                }
            }

            if tray_menu_item(
                ui,
                &pal,
                glyph::QR_CODE,
                pal.on_surface_muted,
                &qr_lbl,
                pal.on_surface,
                false,
                None,
                false,
            ) {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.main_visible = true;
                g.main_want_focus = true;
                g.active_panel = "tcp".to_string();
                drop(g);
                trigger_qr_request(core, &ctx, "tcp");
            }

            paint_divider(ui, &pal);

            let p_ic = if paused {
                glyph::PLAY_ARROW
            } else {
                glyph::PAUSE
            };
            let p_ic_col = if paused {
                pal.amber_pause
            } else {
                pal.on_surface_muted
            };
            if tray_menu_item(
                ui,
                &pal,
                p_ic,
                p_ic_col,
                &pause_lbl,
                pal.on_surface,
                false,
                None,
                false,
            ) {
                core.lock()
                    .unwrap()
                    .send_ipc(IpcCommand::SetPaused { paused: !paused });
            }

            let usb_right = if usb_prio {
                Some((glyph::CHECK, pal.accent))
            } else {
                None
            };
            if tray_menu_item(
                ui,
                &pal,
                glyph::USB,
                pal.on_surface_muted,
                &usb_lbl,
                pal.on_surface,
                false,
                usb_right,
                false,
            ) {
                core.lock().unwrap().send_ipc(IpcCommand::SetUsbPriority {
                    enabled: !usb_prio,
                });
            }

            paint_divider(ui, &pal);

            if tray_menu_item(
                ui,
                &pal,
                glyph::SETTINGS,
                pal.on_surface_muted,
                &opt_lbl,
                pal.on_surface,
                false,
                None,
                false,
            ) {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.options_open = true;
                g.options_just_opened = true;
                g.options_want_focus = true;
            }

            let disc_col = if connected {
                pal.red_disconnect
            } else {
                pal.red_disconnect.gamma_multiply(0.4)
            };
            if tray_menu_item(
                ui,
                &pal,
                glyph::LINK_OFF,
                disc_col,
                &disc_lbl,
                disc_col,
                false,
                None,
                true,
            ) && connected
            {
                core.lock().unwrap().send_ipc(IpcCommand::DisconnectDevice);
            }

            if tray_menu_item(
                ui,
                &pal,
                glyph::POWER_SETTINGS_NEW,
                pal.red_disconnect,
                &exit_lbl,
                pal.red_disconnect,
                false,
                None,
                true,
            ) {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.should_quit = true;
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn tray_menu_item(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    ic: char,
    ic_col: Color32,
    label: &str,
    lbl_col: Color32,
    strong: bool,
    right_ic: Option<(char, Color32)>,
    danger_hover: bool,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            r,
            CornerRadius::same(7),
            if danger_hover {
                pal.red_soft
            } else {
                pal.surface_elevated
            },
        );
    }
    paint_glyph(
        ui.painter(),
        Pos2::new(r.left() + 18.0, r.center().y),
        ic,
        16.0,
        ic_col,
    );
    let font = FontId::proportional(if strong { 13.0 } else { 12.5 });
    ui.painter().text(
        Pos2::new(r.left() + 36.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        lbl_col,
    );
    if let Some((ric, rcol)) = right_ic {
        paint_glyph(
            ui.painter(),
            Pos2::new(r.right() - 14.0, r.center().y),
            ric,
            16.0,
            rcol,
        );
    }
    resp.clicked()
}
