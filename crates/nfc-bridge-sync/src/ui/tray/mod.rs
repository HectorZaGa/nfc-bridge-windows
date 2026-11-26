pub(crate) mod icon;

use super::icons::{
    cap_center_offset_y, glyph, paint_device_os_icon, paint_edit_off_glyph, paint_glyph,
};
use super::main_window::left_column::transport_glyph;
use super::main_window::panels::trigger_qr_request;
use super::state::{SharedUiCore, TraySubmenu};
use super::theme::{apply_win32_dwm_to_title, ThemePalette};
use super::widgets::{paint_divider, paint_inline_capsule};
use crate::ipc::IpcCommand;
#[cfg(windows)]
use crate::platform::{
    configure_tray_popup_win32, hide_tray_popup_win32, is_cursor_in_window_win32,
    poll_tray_input_edges_win32, restore_main_window_win32, show_tray_popup_at_win32,
    show_tray_submenu_at_win32, uncloak_window_win32,
};
use egui::{
    Align, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn estimate_tray_height(g: &SharedUiCore) -> f32 {
    let has_multi = g.live.snapshot.trusted_devices.len() > 1;
    if let Some((hm, measured)) = g.tray_measured_height {
        if hm == has_multi {
            return measured;
        }
    }
    if has_multi {
        417.0_f32
    } else {
        383.0_f32
    }
}

pub(crate) fn estimate_tray_submenu_size(g: &SharedUiCore, kind: TraySubmenu) -> (f32, f32) {
    match kind {
        TraySubmenu::Devices => {
            let count = g.live.snapshot.trusted_devices.len().clamp(1, 6) as f32;
            (218.0, 15.0 + count * 30.0 + (count - 1.0) * 2.0)
        }
        TraySubmenu::Qr => (196.0, 15.0 + 4.0 * 30.0 + 3.0 * 2.0),
    }
}

fn close_tray_and_sub(core: &Arc<Mutex<SharedUiCore>>, tray_title: &str, tray_sub_title: &str) {
    let mut g = core.lock().unwrap();
    g.tray_open = false;
    g.tray_paint_stage = 0;
    g.tray_submenu = None;
    g.tray_sub_paint_stage = 0;
    g.tray_sub_hover_miss_since = None;
    g.tray_sub_switch_pending = None;
    g.tray_closed_at = Some(Instant::now());
    drop(g);
    #[cfg(windows)]
    {
        hide_tray_popup_win32(tray_sub_title);
        hide_tray_popup_win32(tray_title);
    }
}

#[allow(clippy::too_many_arguments)]
fn activate_submenu(
    core: &Arc<Mutex<SharedUiCore>>,
    ctx: &egui::Context,
    tray_title: &str,
    tray_sub_title: &str,
    kind: TraySubmenu,
    row_top: f32,
) {
    let ppp = ctx.pixels_per_point().max(1.0);
    let mut need_open_or_move = false;
    let mut cloak_first = false;
    let mut sub_dims = (212.0, 140.0);
    {
        let mut g = core.lock().unwrap();
        g.tray_sub_hover_miss_since = None;
        g.tray_sub_switch_pending = None;
        let same = g.tray_submenu == Some(kind) && (g.tray_sub_row_y - row_top).abs() < 1.5;
        if !same {
            let was_open = g.tray_submenu.is_some() && g.tray_sub_paint_stage == 0;
            g.tray_submenu = Some(kind);
            g.tray_sub_row_y = row_top;
            cloak_first = !was_open;
            if cloak_first {
                g.tray_sub_paint_stage = 1;
            }
            sub_dims = estimate_tray_submenu_size(&g, kind);
            need_open_or_move = true;
        }
    }
    if need_open_or_move {
        #[cfg(windows)]
        {
            if let Some((lx, ly)) = show_tray_submenu_at_win32(
                tray_sub_title,
                tray_title,
                row_top,
                sub_dims.0,
                sub_dims.1,
                ppp,
                cloak_first,
            ) {
                core.lock().unwrap().tray_sub_pos = Some(Pos2::new(lx, ly));
            }
        }
        ctx.request_repaint_of(egui::ViewportId::from_hash_of("nb_tray_sub_viewport"));
    }
}

pub(crate) fn render_tray_viewport(
    core: &Arc<Mutex<SharedUiCore>>,
    tray_title: &str,
    tray_sub_title: &str,
    ui: &mut egui::Ui,
) {
    let ctx = ui.ctx().clone();
    let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let close_req = ctx.input(|i| i.viewport().close_requested());
    let mut closing_now = false;
    let (stage_was, sub_active_kind, cursor_in_sub_now) = {
        let mut g = core.lock().unwrap();
        if !g.tray_open || g.should_quit {
            drop(g);
            close_tray_and_sub(core, tray_title, tray_sub_title);
            return;
        }
        let stg = g.tray_paint_stage;
        if stg == 2 {
            g.tray_paint_stage = 0;
        }
        let sub_open = g.tray_submenu.is_some();
        let in_main = ctx.input(|i| i.pointer.hover_pos().is_some()) || {
            #[cfg(windows)]
            {
                is_cursor_in_window_win32(tray_title)
            }
            #[cfg(not(windows))]
            {
                false
            }
        };
        let in_sub = sub_open && {
            #[cfg(windows)]
            {
                is_cursor_in_window_win32(tray_sub_title)
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
            || (stg == 0 && !in_main && !in_sub && past_grace && mouse_edge)
        {
            closing_now = true;
        }
        (stg, g.tray_submenu, in_sub)
    };
    if closing_now {
        close_tray_and_sub(core, tray_title, tray_sub_title);
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
        header_os,
        header_mode_glyph,
        header_latency_ms,
        header_device_name,
        header_status_fallback,
        open_lbl,
        switch_lbl,
        has_multi_devs,
        qr_lbl,
        paused,
        pause_lbl,
        read_only_on,
        read_only_lbl,
        usb_en,
        usb_en_lbl,
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
        let active_profile = st
            .selected_device_id
            .as_ref()
            .and_then(|id| {
                st.snapshot
                    .trusted_devices
                    .iter()
                    .find(|d| &d.device_id == id)
            })
            .or_else(|| {
                st.active_device_id.as_ref().and_then(|id| {
                    st.snapshot
                        .trusted_devices
                        .iter()
                        .find(|d| &d.device_id == id)
                })
            })
            .or_else(|| st.snapshot.trusted_devices.first());

        let (l_c, h_os, h_mode, h_ms, h_dev_name, s_fallback) = if !st.driver_online {
            (
                pal.red_disconnect,
                None,
                None,
                None,
                None,
                g.t(if st.driver_installed {
                    "device_driver_stopped"
                } else {
                    "device_driver_not_installed"
                })
                .to_string(),
            )
        } else if st.paused {
            (
                pal.amber_pause,
                st.device_os
                    .clone()
                    .or_else(|| active_profile.and_then(|p| p.device_os.clone()))
                    .or_else(|| Some("android".to_string())),
                None,
                None,
                None,
                g.t(SharedUiCore::paused_reason_key(st.pause_reason.as_deref()))
                    .to_string(),
            )
        } else if st.connected {
            let os = st
                .device_os
                .clone()
                .or_else(|| active_profile.and_then(|p| p.device_os.clone()))
                .unwrap_or_else(|| "android".to_string());
            let dname = st.device_name.clone().unwrap_or_else(|| {
                g.t(if os == "ios" {
                    "device_ios_fallback"
                } else {
                    "device_android_fallback"
                })
                .to_string()
            });
            (
                pal.led_green,
                Some(os),
                Some(transport_glyph(st.transport.as_deref())),
                Some(st.latency_ms),
                Some(dname),
                String::new(),
            )
        } else if let Some(prof) = active_profile {
            (
                pal.on_surface_muted,
                Some(
                    prof.device_os
                        .clone()
                        .unwrap_or_else(|| "android".to_string()),
                ),
                None,
                None,
                None,
                g.t("device_disconnected_short").to_string(),
            )
        } else {
            (
                pal.on_surface_muted,
                None,
                None,
                None,
                None,
                g.t("device_disconnected_short").to_string(),
            )
        };

        let multi = st.snapshot.trusted_devices.len() > 1;
        let paused_val = st.paused;
        let read_only_val = st.snapshot.read_only_mode;
        let usb_en_val = st.snapshot.usb_enabled;
        let usb_prio_val = st.snapshot.usb_auto_connect;
        let conn_val = st.connected;
        (
            pal,
            g.app_display_name(),
            g.t("tray_pc_badge").to_string(),
            l_c,
            h_os,
            h_mode,
            h_ms,
            h_dev_name,
            s_fallback,
            g.t("tray_open_main").to_string(),
            g.t("tray_switch_device").to_string(),
            multi,
            g.t("tray_show_qr").to_string(),
            paused_val,
            g.t(if paused_val {
                "tray_resume_reader"
            } else {
                "tray_pause_reader"
            })
            .to_string(),
            read_only_val,
            g.t("tray_read_only").to_string(),
            usb_en_val,
            g.t("tray_usb_bridge").to_string(),
            usb_prio_val,
            g.t("tray_usb_priority").to_string(),
            g.t("tray_options").to_string(),
            conn_val,
            g.t("tray_disconnect").to_string(),
            g.t("tray_exit").to_string(),
        )
    };

    let mut hovered_submenu_target: Option<(TraySubmenu, f32)> = None;
    let mut clicked_submenu_target: Option<(TraySubmenu, f32)> = None;
    let mut hovered_other_item = false;

    egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
            let content_resp = ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

                // Encabezado con ícono de OS móvil cubriendo ambas líneas
                egui::Frame::new()
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), 38.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                let (ic_r, _) =
                                    ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
                            let (ic_bg, ic_bd, ic_fg) = if connected {
                                (pal.accent_soft, pal.accent_border, pal.accent)
                            } else {
                                (pal.surface_elevated, pal.outline, pal.on_surface_muted)
                            };
                            ui.painter().rect(
                                ic_r,
                                CornerRadius::same(8),
                                ic_bg,
                                Stroke::new(1.0, ic_bd),
                                StrokeKind::Inside,
                            );
                            if let Some(ref os) = header_os {
                                paint_device_os_icon(
                                    ui.painter(),
                                    ic_r.shrink(7.5),
                                    os,
                                    ic_fg,
                                );
                            } else {
                                paint_glyph(
                                    ui.painter(),
                                    ic_r.center(),
                                    glyph::PHONELINK_OFF,
                                    18.0,
                                    pal.on_surface_muted,
                                );
                            }
                            ui.add_space(9.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                                ui.label(
                                    RichText::new(app_name)
                                        .size(13.0)
                                        .strong()
                                        .color(pal.on_surface),
                                );
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                                    let (lr, _) =
                                        ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                                    ui.painter().circle_filled(lr.center(), 3.2, led_color);
                                    if let (Some(dname), Some(m_ic), Some(ms)) = (
                                        header_device_name.as_ref(),
                                        header_mode_glyph,
                                        header_latency_ms,
                                    ) {
                                        ui.label(
                                            RichText::new(dname)
                                                .size(11.0)
                                                .color(pal.on_surface_muted),
                                        );
                                        let (mr, _) = ui.allocate_exact_size(
                                            Vec2::new(13.0, 14.0),
                                            Sense::hover(),
                                        );
                                        paint_glyph(
                                            ui.painter(),
                                            mr.center(),
                                            m_ic,
                                            12.0,
                                            pal.accent,
                                        );
                                        ui.label(
                                            RichText::new(format!("{} ms", ms))
                                                .size(11.0)
                                                .color(pal.on_surface_muted),
                                        );
                                    } else {
                                        ui.label(
                                            RichText::new(&header_status_fallback)
                                                .size(11.0)
                                                .color(pal.on_surface_muted),
                                        );
                                    }
                                });
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                paint_inline_capsule(ui, &pc_badge, true, &pal);
                            });
                        });
                    });

                paint_divider(ui, &pal);

                let open_r = tray_menu_item(
                    ui,
                    &pal,
                    glyph::OPEN_IN_NEW,
                    pal.accent,
                    &open_lbl,
                    pal.on_surface,
                    true,
                    None,
                    false,
                    false,
                );
                if open_r.hovered {
                    hovered_other_item = true;
                }
                if open_r.clicked {
                    close_tray_and_sub(core, tray_title, tray_sub_title);
                    let mut g = core.lock().unwrap();
                    g.main_visible = true;
                    g.main_want_focus = true;
                    let mt = g.app_title.clone();
                    drop(g);
                    #[cfg(windows)]
                    restore_main_window_win32(&mt);
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                }

                if has_multi_devs {
                    let dev_row = tray_menu_item(
                        ui,
                        &pal,
                        glyph::SMARTPHONE,
                        pal.on_surface_muted,
                        &switch_lbl,
                        pal.on_surface,
                        false,
                        Some((glyph::CHEVRON_RIGHT, pal.on_surface_muted)),
                        false,
                        sub_active_kind == Some(TraySubmenu::Devices),
                    );
                    if dev_row.clicked {
                        clicked_submenu_target =
                            Some((TraySubmenu::Devices, dev_row.rect.top()));
                    } else if dev_row.hovered {
                        hovered_submenu_target =
                            Some((TraySubmenu::Devices, dev_row.rect.top()));
                    }
                }

                let qr_row = tray_menu_item(
                    ui,
                    &pal,
                    glyph::QR_CODE,
                    pal.on_surface_muted,
                    &qr_lbl,
                    pal.on_surface,
                    false,
                    Some((glyph::CHEVRON_RIGHT, pal.on_surface_muted)),
                    false,
                    sub_active_kind == Some(TraySubmenu::Qr),
                );
                if qr_row.clicked {
                    clicked_submenu_target = Some((TraySubmenu::Qr, qr_row.rect.top()));
                } else if qr_row.hovered {
                    hovered_submenu_target = Some((TraySubmenu::Qr, qr_row.rect.top()));
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
                let pause_r = tray_menu_item(
                    ui,
                    &pal,
                    p_ic,
                    p_ic_col,
                    &pause_lbl,
                    pal.on_surface,
                    false,
                    None,
                    false,
                    false,
                );
                if pause_r.hovered {
                    hovered_other_item = true;
                }
                if pause_r.clicked {
                    core.lock()
                        .unwrap()
                        .send_ipc(IpcCommand::SetPaused { paused: !paused });
                }

                let ro_right = if read_only_on {
                    Some((glyph::CHECK, pal.amber_pause))
                } else {
                    None
                };
                let ro_r = tray_menu_item_custom(
                    ui,
                    &pal,
                    glyph::EDIT,
                    read_only_on,
                    if read_only_on {
                        pal.amber_pause
                    } else {
                        pal.on_surface_muted
                    },
                    &read_only_lbl,
                    pal.on_surface,
                    false,
                    ro_right,
                    false,
                    false,
                );
                if ro_r.hovered {
                    hovered_other_item = true;
                }
                if ro_r.clicked {
                    core.lock().unwrap().send_ipc(IpcCommand::SetReadOnly {
                        enabled: !read_only_on,
                    });
                }

                let usb_en_right = if usb_en {
                    Some((glyph::CHECK, pal.accent))
                } else {
                    None
                };
                let usb_en_r = tray_menu_item(
                    ui,
                    &pal,
                    if usb_en { glyph::CABLE } else { glyph::USB_OFF },
                    pal.on_surface_muted,
                    &usb_en_lbl,
                    pal.on_surface,
                    false,
                    usb_en_right,
                    false,
                    false,
                );
                if usb_en_r.hovered {
                    hovered_other_item = true;
                }
                if usb_en_r.clicked {
                    core.lock().unwrap().send_ipc(IpcCommand::SetUsbEnabled {
                        enabled: !usb_en,
                    });
                }

                let usb_right = if usb_prio {
                    Some((glyph::CHECK, pal.accent))
                } else {
                    None
                };
                let usb_r = tray_menu_item(
                    ui,
                    &pal,
                    glyph::USB,
                    pal.on_surface_muted,
                    &usb_lbl,
                    pal.on_surface,
                    false,
                    usb_right,
                    false,
                    false,
                );
                if usb_r.hovered {
                    hovered_other_item = true;
                }
                if usb_r.clicked {
                    core.lock().unwrap().send_ipc(IpcCommand::SetUsbPriority {
                        enabled: !usb_prio,
                    });
                }

                paint_divider(ui, &pal);

                let opt_r = tray_menu_item(
                    ui,
                    &pal,
                    glyph::SETTINGS,
                    pal.on_surface_muted,
                    &opt_lbl,
                    pal.on_surface,
                    false,
                    None,
                    false,
                    false,
                );
                if opt_r.hovered {
                    hovered_other_item = true;
                }
                if opt_r.clicked {
                    close_tray_and_sub(core, tray_title, tray_sub_title);
                    core.lock().unwrap().open_options(None);
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                }

                let disc_col = if connected {
                    pal.red_disconnect
                } else {
                    pal.red_disconnect.gamma_multiply(0.4)
                };
                let disc_r = tray_menu_item(
                    ui,
                    &pal,
                    glyph::LINK_OFF,
                    disc_col,
                    &disc_lbl,
                    disc_col,
                    false,
                    None,
                    true,
                    false,
                );
                if disc_r.hovered {
                    hovered_other_item = true;
                }
                if disc_r.clicked && connected {
                    core.lock().unwrap().send_ipc(IpcCommand::DisconnectDevice);
                }

                let exit_r = tray_menu_item(
                    ui,
                    &pal,
                    glyph::POWER_SETTINGS_NEW,
                    pal.red_disconnect,
                    &exit_lbl,
                    pal.red_disconnect,
                    false,
                    None,
                    true,
                    false,
                );
                if exit_r.hovered {
                    hovered_other_item = true;
                }
                if exit_r.clicked {
                    close_tray_and_sub(core, tray_title, tray_sub_title);
                    let mut g = core.lock().unwrap();
                    g.options_open = false;
                    g.should_quit = true;
                    drop(g);
                    ctx.send_viewport_cmd_to(
                        egui::ViewportId::ROOT,
                        egui::ViewportCommand::Close,
                    );
                    ctx.request_repaint_of(egui::ViewportId::ROOT);
                }
            });

            let measured_h = (content_resp.response.rect.height() + 15.0)
                .ceil()
                .clamp(220.0, 560.0);
            let cur_h = ctx.input(|i| {
                i.viewport()
                    .inner_rect
                    .map(|r| r.height())
                    .unwrap_or(0.0)
            });
            if measured_h > 100.0 && (cur_h - measured_h).abs() > 1.0 {
                let mut g = core.lock().unwrap();
                if g.tray_measured_height
                    .is_none_or(|(hm, h)| hm != has_multi_devs || (h - measured_h).abs() > 1.0)
                {
                    g.tray_measured_height = Some((has_multi_devs, measured_h));
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
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
                        296.0, measured_h,
                    )));
                }
            }
            if stage_was == 1 {
                core.lock().unwrap().tray_paint_stage = 2;
                ctx.request_repaint();
            }
        });

    // Gestión de apertura, cambio y cierre suave del submenú en cascada
    if let Some((kind, row_top)) = clicked_submenu_target {
        activate_submenu(core, &ctx, tray_title, tray_sub_title, kind, row_top);
    } else if let Some((kind, row_top)) = hovered_submenu_target {
        if sub_active_kind.is_none() || sub_active_kind == Some(kind) {
            activate_submenu(core, &ctx, tray_title, tray_sub_title, kind, row_top);
        } else {
            let switch_ready = {
                let mut g = core.lock().unwrap();
                g.tray_sub_hover_miss_since = None;
                match g.tray_sub_switch_pending {
                    Some((k, since)) if k == kind => since.elapsed() >= Duration::from_millis(85),
                    _ => {
                        g.tray_sub_switch_pending = Some((kind, Instant::now()));
                        false
                    }
                }
            };
            if switch_ready {
                activate_submenu(core, &ctx, tray_title, tray_sub_title, kind, row_top);
            }
        }
    } else if sub_active_kind.is_some() {
        let should_hide_sub = if cursor_in_sub_now {
            let mut g = core.lock().unwrap();
            g.tray_sub_hover_miss_since = None;
            g.tray_sub_switch_pending = None;
            false
        } else if hovered_other_item {
            let mut g = core.lock().unwrap();
            g.tray_sub_switch_pending = None;
            match g.tray_sub_hover_miss_since {
                Some(since) => since.elapsed() >= Duration::from_millis(120),
                None => {
                    g.tray_sub_hover_miss_since = Some(Instant::now());
                    false
                }
            }
        } else {
            false
        };
        if should_hide_sub {
            {
                let mut g = core.lock().unwrap();
                g.tray_submenu = None;
                g.tray_sub_paint_stage = 0;
                g.tray_sub_hover_miss_since = None;
                g.tray_sub_switch_pending = None;
            }
            #[cfg(windows)]
            hide_tray_popup_win32(tray_sub_title);
        }
    }
}

pub(crate) fn render_tray_sub_viewport(
    core: &Arc<Mutex<SharedUiCore>>,
    tray_title: &str,
    tray_sub_title: &str,
    ui: &mut egui::Ui,
) {
    let ctx = ui.ctx().clone();
    let (stage_was, sub_kind, row_y, pal, devices, active_id, selected_id, qr_opts) = {
        let mut g = core.lock().unwrap();
        let Some(kind) = g.tray_submenu else {
            drop(g);
            #[cfg(windows)]
            hide_tray_popup_win32(tray_sub_title);
            return;
        };
        if !g.tray_open || g.should_quit {
            drop(g);
            #[cfg(windows)]
            hide_tray_popup_win32(tray_sub_title);
            return;
        }
        let stg = g.tray_sub_paint_stage;
        if stg == 2 {
            g.tray_sub_paint_stage = 0;
        }
        let pal = g.palette();
        let dwm_key = (pal.is_dark, pal.outline);
        if g.last_dwm_tray_sub_key != Some(dwm_key) {
            g.last_dwm_tray_sub_key = Some(dwm_key);
            apply_win32_dwm_to_title(tray_sub_title, &pal);
            #[cfg(windows)]
            configure_tray_popup_win32(tray_sub_title);
        }
        let drv_on = g.live.driver_online;
        let relay_has_url = g
            .live
            .snapshot
            .relay_url
            .as_deref()
            .is_some_and(|u| !u.trim().is_empty());
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
            stg,
            kind,
            g.tray_sub_row_y,
            pal,
            g.live.snapshot.trusted_devices.clone(),
            g.live.active_device_id.clone(),
            g.live.selected_device_id.clone(),
            qr_opts,
        )
    };

    if stage_was == 2 {
        #[cfg(windows)]
        uncloak_window_win32(tray_sub_title, false);
    }

    let sub_w = match sub_kind {
        TraySubmenu::Devices => 218.0_f32,
        TraySubmenu::Qr => 196.0_f32,
    };

    egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
            let content_resp = ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                match sub_kind {
                    TraySubmenu::Devices => {
                        for dev in devices.iter().take(6) {
                            let is_act = Some(&dev.device_id) == active_id.as_ref()
                                || Some(&dev.device_id) == selected_id.as_ref();
                            let (r, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 30.0),
                                Sense::click(),
                            );
                            if is_act || resp.hovered() {
                                ui.painter().rect_filled(
                                    r,
                                    CornerRadius::same(8),
                                    pal.surface_elevated,
                                );
                            }
                            let name_col = if is_act {
                                pal.on_surface
                            } else {
                                pal.on_surface_muted
                            };
                            let name_gal = ui.painter().layout_no_wrap(
                                dev.device_name.clone(),
                                FontId::proportional(12.0),
                                name_col,
                            );
                            let max_name_w = (r.width() - 48.0).max(60.0);
                            let name_w = name_gal.size().x.min(max_name_w);
                            let name_pos = Pos2::new(
                                r.left() + 10.0,
                                r.center().y - name_gal.size().y * 0.5,
                            );
                            let name_clip = Rect::from_min_max(
                                Pos2::new(r.left() + 10.0, r.top()),
                                Pos2::new(r.left() + 10.0 + max_name_w, r.bottom()),
                            );
                            ui.painter()
                                .with_clip_rect(name_clip)
                                .galley(name_pos, name_gal, name_col);
                            paint_glyph(
                                ui.painter(),
                                Pos2::new(r.left() + 10.0 + name_w + 11.0, r.center().y),
                                transport_glyph(dev.transport.as_deref()),
                                13.5,
                                if is_act {
                                    pal.accent
                                } else {
                                    pal.on_surface_muted
                                },
                            );
                            if is_act {
                                paint_glyph(
                                    ui.painter(),
                                    Pos2::new(r.right() - 12.0, r.center().y),
                                    glyph::CHECK,
                                    14.0,
                                    pal.led_green,
                                );
                            }
                            if resp.clicked() {
                                let id = dev.device_id.clone();
                                close_tray_and_sub(core, tray_title, tray_sub_title);
                                let g = core.lock().unwrap();
                                let ipc = g.ipc.clone();
                                g.rt.spawn(async move {
                                    ipc.select_saved_profile(&id).await;
                                    let _ = ipc
                                        .send_command(IpcCommand::SelectTrustedDevice {
                                            device_id: id,
                                        })
                                        .await;
                                });
                                ctx.request_repaint_of(egui::ViewportId::ROOT);
                            }
                        }
                    }
                    TraySubmenu::Qr => {
                        for (method_key, m_ic, m_lbl, can_generate) in qr_opts {
                            let (r, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 30.0),
                                Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(
                                    r,
                                    CornerRadius::same(8),
                                    pal.surface_elevated,
                                );
                            }
                            paint_glyph(
                                ui.painter(),
                                Pos2::new(r.left() + 14.0, r.center().y),
                                m_ic,
                                15.0,
                                if can_generate {
                                    pal.accent
                                } else {
                                    pal.on_surface_muted
                                },
                            );
                            ui.painter().text(
                                Pos2::new(r.left() + 30.0, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                &m_lbl,
                                FontId::proportional(12.0),
                                if can_generate {
                                    pal.on_surface
                                } else {
                                    pal.on_surface_muted
                                },
                            );
                            if resp.clicked() {
                                close_tray_and_sub(core, tray_title, tray_sub_title);
                                let mut g = core.lock().unwrap();
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
                                restore_main_window_win32(&mt);
                                if can_generate {
                                    trigger_qr_request(core, &ctx, method_key);
                                }
                                ctx.request_repaint_of(egui::ViewportId::ROOT);
                            }
                        }
                    }
                }
            });

            let measured_h = (content_resp.response.rect.height() + 15.0)
                .ceil()
                .clamp(48.0, 320.0);
            let cur_h = ctx.input(|i| {
                i.viewport()
                    .inner_rect
                    .map(|r| r.height())
                    .unwrap_or(0.0)
            });
            if measured_h > 40.0 && (cur_h - measured_h).abs() > 1.0 {
                #[cfg(windows)]
                {
                    let ppp = ctx.pixels_per_point().max(1.0);
                    if let Some((lx, ly)) = show_tray_submenu_at_win32(
                        tray_sub_title,
                        tray_title,
                        row_y,
                        sub_w,
                        measured_h,
                        ppp,
                        false,
                    ) {
                        core.lock().unwrap().tray_sub_pos = Some(Pos2::new(lx, ly));
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
                    sub_w, measured_h,
                )));
            }
            if stage_was == 1 {
                core.lock().unwrap().tray_sub_paint_stage = 2;
                ctx.request_repaint();
            }
        });
}

struct TrayRowResp {
    clicked: bool,
    hovered: bool,
    rect: Rect,
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
    force_active_bg: bool,
) -> TrayRowResp {
    tray_menu_item_custom(
        ui,
        pal,
        ic,
        false,
        ic_col,
        label,
        lbl_col,
        strong,
        right_ic,
        danger_hover,
        force_active_bg,
    )
}

#[allow(clippy::too_many_arguments)]
fn tray_menu_item_custom(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    ic: char,
    ic_crossed_out: bool,
    ic_col: Color32,
    label: &str,
    lbl_col: Color32,
    strong: bool,
    right_ic: Option<(char, Color32)>,
    danger_hover: bool,
    force_active_bg: bool,
) -> TrayRowResp {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    let hover_bg = if resp.hovered() || force_active_bg {
        if danger_hover {
            pal.red_soft
        } else {
            pal.surface_elevated
        }
    } else {
        pal.surface_card
    };
    if resp.hovered() || force_active_bg {
        ui.painter().rect_filled(r, CornerRadius::same(8), hover_bg);
    }
    let ic_center = Pos2::new(r.left() + 18.0, r.center().y);
    if ic_crossed_out && ic == glyph::EDIT {
        paint_edit_off_glyph(ui.painter(), ic_center, 16.0, ic_col, hover_bg);
    } else {
        paint_glyph(ui.painter(), ic_center, ic, 16.0, ic_col);
    }
    let font = FontId::proportional(if strong { 13.0 } else { 12.5 });
    let dy = cap_center_offset_y(ui.painter(), &font);
    let gal = ui.painter().layout_no_wrap(label.to_string(), font, lbl_col);
    ui.painter()
        .galley(Pos2::new(r.left() + 36.0, r.center().y - dy), gal, lbl_col);
    if let Some((ric, rcol)) = right_ic {
        paint_glyph(
            ui.painter(),
            Pos2::new(r.right() - 14.0, r.center().y),
            ric,
            16.0,
            rcol,
        );
    }
    TrayRowResp {
        clicked: resp.clicked(),
        hovered: resp.hovered(),
        rect: r,
    }
}
