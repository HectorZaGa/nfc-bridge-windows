pub(crate) mod icon;

use super::icons::{glyph, paint_glyph};
use super::main_window::panels::trigger_qr_request;
use super::state::SharedUiCore;
use super::theme::{apply_win32_dwm_to_title, ThemePalette};
use super::widgets::{paint_divider, paint_inline_capsule};
use crate::ipc::IpcCommand;
#[cfg(windows)]
use crate::platform::{
    configure_tray_popup_win32, hide_tray_popup_win32, is_cursor_in_window_win32,
    poll_tray_input_edges_win32, restore_main_window_win32, show_tray_popup_at_win32,
    uncloak_window_win32,
};
use egui::{
    Align, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) fn estimate_tray_height(g: &SharedUiCore) -> f32 {
    let dev_count = g.live.snapshot.trusted_devices.len().min(4);
    let has_multi = g.live.snapshot.trusted_devices.len() > 1;
    let dev_exp = has_multi && g.tray_devices_expanded;
    let qr_exp = g.tray_qr_expanded;
    if let Some((hm, de, cnt, qe, measured)) = g.tray_measured_height {
        if hm == has_multi && de == dev_exp && cnt == dev_count && qe == qr_exp {
            return measured;
        }
    }
    let mut base = if has_multi { 336.0_f32 } else { 302.0_f32 };
    if dev_exp {
        base += (dev_count as f32) * 30.0;
    }
    if qr_exp {
        base += 4.0 * 30.0;
    }
    base
}

pub(crate) fn render_tray_viewport(
    core: &Arc<Mutex<SharedUiCore>>,
    tray_title: &str,
    ui: &mut egui::Ui,
) {
    let ctx = ui.ctx().clone();
    let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let close_req = ctx.input(|i| i.viewport().close_requested());
    let mut closing_now = false;
    let stage_was = {
        let mut g = core.lock().unwrap();
        if !g.tray_open || g.should_quit {
            g.tray_open = false;
            g.tray_paint_stage = 0;
            drop(g);
            #[cfg(windows)]
            hide_tray_popup_win32(tray_title);
            return;
        }
        let stg = g.tray_paint_stage;
        if stg == 2 {
            g.tray_paint_stage = 0;
        }
        let pointer_in_rect = ctx.input(|i| i.pointer.hover_pos().is_some()) || {
            #[cfg(windows)]
            {
                is_cursor_in_window_win32(tray_title)
            }
            #[cfg(not(windows))]
            {
                false
            }
        };
        let (mouse_edge, esc_edge) = {
            #[cfg(windows)]
            {
                poll_tray_input_edges_win32(&mut g.tray_input_mask)
            }
            #[cfg(not(windows))]
            {
                (false, false)
            }
        };
        let past_grace = g
            .tray_opened_at
            .is_some_and(|t| t.elapsed() > Duration::from_millis(180));
        if esc_pressed
            || esc_edge
            || close_req
            || (stg == 0 && !pointer_in_rect && past_grace && mouse_edge)
        {
            g.tray_open = false;
            g.tray_paint_stage = 0;
            g.tray_closed_at = Some(std::time::Instant::now());
            closing_now = true;
        }
        stg
    };
    if closing_now {
        #[cfg(windows)]
        hide_tray_popup_win32(tray_title);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
        return;
    }
    if stage_was == 2 {
        #[cfg(windows)]
        uncloak_window_win32(tray_title, false);
    }
    ctx.request_repaint_after(Duration::from_millis(25));

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
        qr_expanded,
        qr_lbl,
        qr_options,
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
            #[cfg(windows)]
            configure_tray_popup_win32(tray_title);
        }
        let st = &g.live;
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
        let has_multi_devs = g.live.snapshot.trusted_devices.len() > 1;
        let relay_has_url = g
            .live
            .snapshot
            .relay_url
            .as_deref()
            .is_some_and(|u| !u.trim().is_empty());
        let drv_on = g.live.driver_online;
        let devices = g.live.snapshot.trusted_devices.clone();
        let active_id = g.live.active_device_id.clone();
        let selected_id = g.live.selected_device_id.clone();
        let paused = g.live.paused;
        let usb_prio = g.live.snapshot.usb_auto_connect;
        let connected = g.live.connected;
        if !has_multi_devs {
            g.tray_devices_expanded = false;
        }
        let qr_opts = vec![
            ("tcp", glyph::WIFI, g.t("method_tcp_title").to_string(), drv_on),
            (
                "bluetooth",
                glyph::BLUETOOTH,
                g.t("method_bt_title").to_string(),
                drv_on,
            ),
            (
                "relay",
                glyph::LINK,
                g.t("method_relay_title").to_string(),
                drv_on && relay_has_url,
            ),
            (
                "wifi_direct",
                glyph::WIFI_TETHERING,
                g.t("method_wfd_title").to_string(),
                drv_on,
            ),
        ];
        (
            pal,
            g.app_display_name(),
            g.t("tray_pc_badge").to_string(),
            l_c,
            s_txt,
            g.t("tray_open_main").to_string(),
            g.t("tray_switch_device").to_string(),
            devices,
            active_id,
            selected_id,
            g.tray_devices_expanded && has_multi_devs,
            g.tray_qr_expanded,
            g.t("tray_show_qr").to_string(),
            qr_opts,
            paused,
            g.t(if paused {
                "tray_resume_reader"
            } else {
                "tray_pause_reader"
            })
            .to_string(),
            usb_prio,
            g.t("tray_usb_priority").to_string(),
            g.t("tray_options").to_string(),
            connected,
            g.t("tray_disconnect").to_string(),
            g.t("tray_exit").to_string(),
        )
    };

    let has_multi_devs = devices.len() > 1;
    let dev_count = devices.len().min(4);
    egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
            let content_resp = ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                egui::Frame::new()
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        ui.painter().rect(
                            ic_r,
                            CornerRadius::same(8),
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
            ) && !closing_now
            {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.tray_closed_at = Some(std::time::Instant::now());
                g.main_visible = true;
                g.main_want_focus = true;
                let mt = g.app_title.clone();
                drop(g);
                #[cfg(windows)]
                {
                    hide_tray_popup_win32(tray_title);
                    restore_main_window_win32(&mt);
                }
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }

            if has_multi_devs {
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
                ) && !closing_now
                {
                    let mut g = core.lock().unwrap();
                    g.tray_devices_expanded = !g.tray_devices_expanded;
                    if g.tray_devices_expanded {
                        g.tray_qr_expanded = false;
                    }
                    let next_h = estimate_tray_height(&g);
                    #[cfg(windows)]
                    if let Some((ax, ay)) = g.tray_anchor_px {
                        let ppp = ctx.pixels_per_point().max(1.0);
                        if let Some((lx, ly)) = show_tray_popup_at_win32(
                            tray_title, ax, ay, 296.0, next_h, ppp, false,
                        ) {
                            g.tray_pos = Some(Pos2::new(lx, ly));
                        }
                    }
                    drop(g);
                    ctx.request_repaint();
                }
                if dev_expanded {
                    for dev in devices.iter().take(4) {
                        let is_act = Some(&dev.device_id) == active_id.as_ref()
                            || Some(&dev.device_id) == selected_id.as_ref();
                        let (r, resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 28.0),
                            Sense::click(),
                        );
                        let inner = Rect::from_min_max(Pos2::new(r.left() + 26.0, r.top()), r.max);
                        if is_act || resp.hovered() {
                            ui.painter().rect_filled(
                                inner,
                                CornerRadius::same(8),
                                pal.surface_elevated,
                            );
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
            }

            let qr_chev = if qr_expanded {
                glyph::EXPAND_MORE
            } else {
                glyph::CHEVRON_RIGHT
            };
            if tray_menu_item(
                ui,
                &pal,
                glyph::QR_CODE,
                pal.on_surface_muted,
                &qr_lbl,
                pal.on_surface,
                false,
                Some((qr_chev, pal.on_surface_muted)),
                false,
            ) && !closing_now
            {
                let mut g = core.lock().unwrap();
                g.tray_qr_expanded = !g.tray_qr_expanded;
                if g.tray_qr_expanded {
                    g.tray_devices_expanded = false;
                }
                let next_h = estimate_tray_height(&g);
                #[cfg(windows)]
                if let Some((ax, ay)) = g.tray_anchor_px {
                    let ppp = ctx.pixels_per_point().max(1.0);
                    if let Some((lx, ly)) =
                        show_tray_popup_at_win32(tray_title, ax, ay, 296.0, next_h, ppp, false)
                    {
                        g.tray_pos = Some(Pos2::new(lx, ly));
                    }
                }
                drop(g);
                ctx.request_repaint();
            }
            if qr_expanded {
                for (method_key, m_ic, m_lbl, can_generate) in qr_options {
                    let (r, resp) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
                    let inner = Rect::from_min_max(Pos2::new(r.left() + 26.0, r.top()), r.max);
                    if resp.hovered() {
                        ui.painter()
                            .rect_filled(inner, CornerRadius::same(8), pal.surface_elevated);
                    }
                    paint_glyph(
                        ui.painter(),
                        Pos2::new(inner.left() + 14.0, inner.center().y),
                        m_ic,
                        14.0,
                        if can_generate {
                            pal.accent
                        } else {
                            pal.on_surface_muted
                        },
                    );
                    ui.painter().text(
                        Pos2::new(inner.left() + 28.0, inner.center().y),
                        egui::Align2::LEFT_CENTER,
                        &m_lbl,
                        FontId::proportional(11.5),
                        if can_generate {
                            pal.on_surface
                        } else {
                            pal.on_surface_muted
                        },
                    );
                    if resp.clicked() && !closing_now {
                        let mut g = core.lock().unwrap();
                        g.tray_open = false;
                        g.tray_paint_stage = 0;
                        g.tray_qr_expanded = false;
                        g.tray_closed_at = Some(std::time::Instant::now());
                        g.main_visible = true;
                        g.main_want_focus = true;
                        g.active_panel = method_key.to_string();
                        if !can_generate {
                            g.qr_method = None;
                            g.qr_error = None;
                        }
                        let mt = g.app_title.clone();
                        drop(g);
                        #[cfg(windows)]
                        {
                            hide_tray_popup_win32(tray_title);
                            restore_main_window_win32(&mt);
                        }
                        if can_generate {
                            trigger_qr_request(core, &ctx, method_key);
                        }
                        ctx.request_repaint_of(egui::ViewportId::ROOT);
                    }
                }
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
            ) && !closing_now
            {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.tray_paint_stage = 0;
                g.tray_closed_at = Some(std::time::Instant::now());
                g.open_options(None);
                drop(g);
                #[cfg(windows)]
                hide_tray_popup_win32(tray_title);
                ctx.request_repaint_of(egui::ViewportId::ROOT);
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
                && !closing_now
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
            ) && !closing_now
            {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.options_open = false;
                g.should_quit = true;
                drop(g);
                #[cfg(windows)]
                hide_tray_popup_win32(tray_title);
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
            });

            let measured_h = (content_resp.response.rect.height() + 12.0).ceil();
            let cur_h = ctx.input(|i| {
                i.viewport()
                    .inner_rect
                    .map(|r| r.height())
                    .unwrap_or(0.0)
            });
            if measured_h > 100.0 && (cur_h - measured_h).abs() > 1.0 && !closing_now {
                let mut g = core.lock().unwrap();
                if g.tray_measured_height.is_none_or(|(hm, de, cnt, qe, h)| {
                    hm != has_multi_devs
                        || de != dev_expanded
                        || cnt != dev_count
                        || qe != qr_expanded
                        || (h - measured_h).abs() > 1.0
                }) {
                    g.tray_measured_height =
                        Some((has_multi_devs, dev_expanded, dev_count, qr_expanded, measured_h));
                    #[cfg(windows)]
                    if let Some((ax, ay)) = g.tray_anchor_px {
                        let ppp = ctx.pixels_per_point().max(1.0);
                        if let Some((lx, ly)) = show_tray_popup_at_win32(
                            tray_title, ax, ay, 296.0, measured_h, ppp, false,
                        ) {
                            g.tray_pos = Some(Pos2::new(lx, ly));
                        }
                    }
                    drop(g);
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(296.0, measured_h)));
                }
            }
            if stage_was == 1 {
                core.lock().unwrap().tray_paint_stage = 2;
                ctx.request_repaint();
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
            CornerRadius::same(8),
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
