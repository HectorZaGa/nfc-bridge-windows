use super::super::icons::{glyph, paint_chevron_y, paint_device_os_icon, paint_glyph, semibold};
use super::super::state::{OptionsTab, SharedUiCore};
use super::super::theme::ThemePalette;
use super::super::widgets::{paint_divider, small_accent_btn};
use crate::ipc::IpcCommand;
use egui::{
    Align, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use std::sync::{Arc, Mutex};

pub(crate) fn render_left_column(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    active_panel: &str,
) -> (Rect, Rect) {
    let (
        header_count_txt,
        sec_mobile_lbl,
        sec_conn_lbl,
        sec_conn_hint,
        device_name,
        device_os,
        device_meta,
        led_color,
        transport_icon,
        has_dropdown,
        dropdown_open,
        is_connected,
        is_paused,
        pause_lbl,
        install_drv_lbl,
        driver_online,
        driver_installed,
        lang,
    ) = {
        let g = core.lock().unwrap();
        let st = &g.live;
        let snap = &st.snapshot;
        let count_txt = if st.connected {
            if snap.trusted_devices.len() > 1 {
                g.t_vars(
                    "devices_1_connected_saved",
                    &[("count", &snap.trusted_devices.len().to_string())],
                )
            } else {
                g.t("devices_1_connected").to_string()
            }
        } else if snap.trusted_devices.len() == 1 {
            g.t("devices_n_saved_one").to_string()
        } else if snap.trusted_devices.len() > 1 {
            g.t_vars(
                "devices_n_saved_many",
                &[("count", &snap.trusted_devices.len().to_string())],
            )
        } else {
            g.t("devices_0_connected").to_string()
        };

        let active_profile = st
            .selected_device_id
            .as_deref()
            .and_then(|id| snap.trusted_devices.iter().find(|d| d.device_id == id))
            .or_else(|| snap.trusted_devices.first());

        let (d_name, d_os, d_meta, l_col, tr_ic) = if !st.driver_online {
            if !st.driver_installed {
                (
                    g.t("device_driver_not_installed").to_string(),
                    "android".to_string(),
                    g.t("device_driver_install_hint").to_string(),
                    pal.red_disconnect,
                    None,
                )
            } else {
                (
                    g.t("device_driver_stopped").to_string(),
                    "android".to_string(),
                    g.t("device_driver_start_hint").to_string(),
                    pal.amber_pause,
                    None,
                )
            }
        } else if st.connected {
            let os = st.device_os.clone().unwrap_or_else(|| "android".to_string());
            let name = st.device_name.clone().unwrap_or_else(|| {
                g.t(if os == "ios" {
                    "device_ios_fallback"
                } else {
                    "device_android_fallback"
                })
                .to_string()
            });
            let meta = if st.paused {
                g.t("device_reader_paused").to_string()
            } else {
                g.t_vars(
                    "device_reader_active_ms",
                    &[("ms", &st.latency_ms.to_string())],
                )
            };
            let led = if st.paused {
                pal.amber_pause
            } else {
                pal.led_green
            };
            (name, os, meta, led, Some(transport_glyph(st.transport.as_deref())))
        } else if let Some(prof) = active_profile {
            (
                prof.device_name.clone(),
                prof.device_os.clone().unwrap_or_else(|| "android".to_string()),
                g.t("device_saved_disconnected").to_string(),
                pal.red_disconnect,
                Some(transport_glyph(prof.transport.as_deref())),
            )
        } else {
            (
                g.t("device_none_title").to_string(),
                "android".to_string(),
                g.t("device_none_subtitle").to_string(),
                pal.on_surface_muted,
                None,
            )
        };

        (
            count_txt,
            g.t("section_mobile_device").to_string(),
            g.t("section_connection_options").to_string(),
            g.t("connection_options_hint").to_string(),
            d_name,
            d_os,
            d_meta,
            l_col,
            tr_ic,
            !snap.trusted_devices.is_empty(),
            g.device_dropdown_open,
            st.connected,
            st.paused,
            g.t(if st.paused { "btn_resume" } else { "btn_pause" }).to_string(),
            g.t("btn_go_install_driver").to_string(),
            st.driver_online,
            st.driver_installed,
            g.current_lang().to_string(),
        )
    };

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(sec_mobile_lbl)
                .font(semibold(16.0))
                .color(pal.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(header_count_txt)
                    .size(12.0)
                    .color(pal.on_surface_muted),
            );
        });
    });
    ui.add_space(4.0);

    let mut sel_btn_rect = Rect::NOTHING;
    let card_resp = egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            let full_w = ui.available_width();
            let lbl_gal = ui.painter().layout_no_wrap(
                pause_lbl.clone(),
                FontId::proportional(12.0),
                pal.on_surface,
            );
            let pause_btn_w = (lbl_gal.size().x + 38.0).clamp(74.0, 92.0);
            let right_controls_w = 6.0 + pause_btn_w + 6.0 + 32.0 + 6.0;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let hover_w = (full_w - right_controls_w).max(150.0);
                let (hover_rect, _) = ui.allocate_exact_size(
                    Vec2::new(hover_w, 48.0),
                    Sense::hover(),
                );
                let sel_resp = ui.interact(
                    hover_rect,
                    egui::Id::unique("active_dev_dd_btn"),
                    if has_dropdown {
                        Sense::click()
                    } else {
                        Sense::hover()
                    },
                );
                sel_btn_rect = hover_rect;
                if has_dropdown && sel_resp.hovered() {
                    ui.painter()
                        .rect_filled(hover_rect, CornerRadius::same(8), pal.surface_elevated);
                }

                let content_r = hover_rect.shrink2(Vec2::new(8.0, 4.0));
                let icon_box = Rect::from_min_size(content_r.min, Vec2::splat(40.0));
                let (ic_bg, ic_border, ic_fg) = if is_connected {
                    (pal.accent_soft, pal.accent_border, pal.accent)
                } else {
                    (pal.surface_elevated, pal.outline, pal.on_surface_muted)
                };
                ui.painter().rect(
                    icon_box,
                    CornerRadius::same(8),
                    ic_bg,
                    Stroke::new(1.0, ic_border),
                    StrokeKind::Inside,
                );
                if driver_online && (is_connected || has_dropdown) {
                    paint_device_os_icon(
                        ui.painter(),
                        icon_box.shrink(8.0),
                        &device_os,
                        ic_fg,
                    );
                } else {
                    paint_glyph(
                        ui.painter(),
                        icon_box.center(),
                        glyph::PHONELINK_OFF,
                        18.0,
                        pal.on_surface_muted,
                    );
                }

                let tx_x = icon_box.right() + 8.0;
                let name_gal = ui
                    .painter()
                    .layout_no_wrap(device_name, semibold(16.0), pal.on_surface);
                let name_w = name_gal.size().x.min(hover_w - 122.0);
                ui.painter()
                    .galley(Pos2::new(tx_x, content_r.top() + 1.0), name_gal, pal.on_surface);

                let line1_cy = content_r.top() + 11.0;
                let mut badge_x = tx_x + name_w + 8.0;
                ui.painter()
                    .circle_filled(Pos2::new(badge_x, line1_cy), 4.0, led_color);
                badge_x += 10.0;

                if let Some(tic) = transport_icon {
                    let cap = Rect::from_center_size(
                        Pos2::new(badge_x + 10.0, line1_cy),
                        Vec2::new(24.0, 19.0),
                    );
                    let (cap_bg, cap_stroke, cap_fg) = if is_connected {
                        (pal.accent_soft, pal.accent_border, pal.accent_on_soft)
                    } else {
                        (pal.surface_elevated, pal.outline, pal.on_surface_muted)
                    };
                    ui.painter().rect(
                        cap,
                        CornerRadius::same(8),
                        cap_bg,
                        Stroke::new(1.0, cap_stroke),
                        StrokeKind::Inside,
                    );
                    paint_glyph(ui.painter(), cap.center(), tic, 13.0, cap_fg);
                    badge_x += 25.0;
                }
                if has_dropdown {
                    paint_chevron_y(
                        ui.painter(),
                        Pos2::new(badge_x + 7.0, line1_cy),
                        dropdown_open,
                        15.0,
                        pal.on_surface_muted,
                    );
                }

                ui.painter().text(
                    Pos2::new(tx_x, content_r.top() + 23.0),
                    egui::Align2::LEFT_TOP,
                    device_meta,
                    FontId::proportional(12.0),
                    pal.on_surface_muted,
                );

                if has_dropdown && sel_resp.clicked() {
                    let mut g = core.lock().unwrap();
                    g.device_dropdown_open = !g.device_dropdown_open;
                }

                ui.add_space(6.0);

                if !driver_online && !driver_installed {
                    let action_w = (pause_btn_w + 6.0 + 32.0).max(112.0);
                    if small_accent_btn(ui, pal, &install_drv_lbl, action_w, 32.0) {
                        core.lock().unwrap().open_options(Some(OptionsTab::Updates));
                    }
                } else {
                    let (p_rect, p_resp) = ui.allocate_exact_size(
                        Vec2::new(pause_btn_w, 32.0),
                        if is_connected {
                            Sense::click()
                        } else {
                            Sense::hover()
                        },
                    );
                    let (p_bg, p_stroke, p_fg, p_ic) = if !is_connected {
                        (
                            pal.surface_elevated.gamma_multiply(0.35),
                            pal.outline.gamma_multiply(0.35),
                            pal.on_surface.gamma_multiply(0.35),
                            glyph::PAUSE,
                        )
                    } else if is_paused {
                        (
                            pal.amber_soft,
                            pal.amber_pause,
                            pal.on_amber_soft,
                            glyph::PLAY_ARROW,
                        )
                    } else {
                        (
                            if p_resp.hovered() {
                                pal.outline
                            } else {
                                pal.surface_elevated
                            },
                            pal.outline,
                            pal.on_surface,
                            glyph::PAUSE,
                        )
                    };
                    ui.painter().rect(
                        p_rect,
                        CornerRadius::same(8),
                        p_bg,
                        Stroke::new(1.0, p_stroke),
                        StrokeKind::Inside,
                    );
                    let p_ic_col = if is_connected && is_paused {
                        pal.amber_pause
                    } else if is_connected {
                        pal.on_surface_muted
                    } else {
                        pal.on_surface_muted.gamma_multiply(0.35)
                    };
                    paint_glyph(
                        ui.painter(),
                        Pos2::new(p_rect.left() + 14.0, p_rect.center().y),
                        p_ic,
                        16.0,
                        p_ic_col,
                    );
                    ui.painter().text(
                        Pos2::new(p_rect.left() + 26.0, p_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        pause_lbl,
                        FontId::proportional(12.0),
                        p_fg,
                    );
                    if is_connected && p_resp.clicked() {
                        core.lock()
                            .unwrap()
                            .send_ipc(IpcCommand::SetPaused { paused: !is_paused });
                    }

                    ui.add_space(6.0);

                    let (d_rect, d_resp) = ui.allocate_exact_size(
                        Vec2::splat(32.0),
                        if is_connected {
                            Sense::click()
                        } else {
                            Sense::hover()
                        },
                    );
                    let d_alpha = if is_connected { 1.0 } else { 0.35 };
                    ui.painter().rect(
                        d_rect,
                        CornerRadius::same(8),
                        if is_connected && d_resp.hovered() {
                            pal.red_soft
                        } else {
                            Color32::TRANSPARENT
                        },
                        Stroke::new(1.0, pal.red_border.gamma_multiply(d_alpha)),
                        StrokeKind::Inside,
                    );
                    paint_glyph(
                        ui.painter(),
                        d_rect.center(),
                        glyph::LINK_OFF,
                        16.0,
                        pal.red_disconnect.gamma_multiply(d_alpha),
                    );
                    if is_connected && d_resp.clicked() {
                        core.lock().unwrap().send_ipc(IpcCommand::DisconnectDevice);
                    }
                }
            });
        });

    ui.add_space(12.0);
    paint_divider(ui, pal);
    ui.add_space(12.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(sec_conn_lbl)
                .font(semibold(16.0))
                .color(pal.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(sec_conn_hint)
                    .size(11.0)
                    .color(pal.on_surface_muted),
            );
        });
    });
    ui.add_space(4.0);

    let rows = [
        ("usb", glyph::USB, "method_usb_title", "method_usb_desc"),
        ("tcp", glyph::WIFI, "method_tcp_title", "method_tcp_desc"),
        (
            "bluetooth",
            glyph::BLUETOOTH,
            "method_bt_title",
            "method_bt_desc",
        ),
        (
            "relay",
            glyph::LINK,
            "method_relay_title",
            "method_relay_desc",
        ),
        (
            "wifi_direct",
            glyph::WIFI_TETHERING,
            "method_wfd_title",
            "method_wfd_desc",
        ),
    ];

    let gap_y = 6.0;
    let card_h = ((ui.available_height() - gap_y * 4.0) / 5.0).clamp(48.0, 53.0);
    for (idx, (key, ic, tk, dk)) in rows.iter().enumerate() {
        let selected = active_panel == *key;
        let (title, desc) = {
            let g = core.lock().unwrap();
            (
                g.i18n.t(&lang, tk).to_string(),
                g.i18n.t(&lang, dk).to_string(),
            )
        };
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), card_h), Sense::click());
        let (bg, border) = if selected {
            (
                if resp.hovered() {
                    pal.accent_soft
                } else {
                    pal.accent_card_bg
                },
                pal.accent_border,
            )
        } else {
            (
                if resp.hovered() {
                    pal.surface_elevated
                } else {
                    pal.surface_card
                },
                pal.outline,
            )
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(8),
            bg,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );

        paint_glyph(
            ui.painter(),
            Pos2::new(rect.left() + 21.0, rect.center().y),
            *ic,
            18.0,
            pal.accent,
        );

        let tx = rect.left() + 38.0;
        ui.painter().text(
            Pos2::new(tx, rect.top() + 8.0),
            egui::Align2::LEFT_TOP,
            title,
            semibold(13.5),
            pal.on_surface,
        );
        ui.painter().text(
            Pos2::new(tx, rect.top() + 27.0),
            egui::Align2::LEFT_TOP,
            desc,
            FontId::proportional(11.0),
            pal.on_surface_muted,
        );

        paint_glyph(
            ui.painter(),
            Pos2::new(rect.right() - 18.0, rect.center().y),
            glyph::CHEVRON_RIGHT,
            16.0,
            if selected {
                pal.accent
            } else {
                pal.on_surface_muted
            },
        );

        if resp.clicked() {
            let mut g = core.lock().unwrap();
            g.active_panel = (*key).to_string();
            g.qr_method = None;
            g.qr_error = None;
        }
        if idx < rows.len() - 1 {
            ui.add_space(gap_y);
        }
    }

    (card_resp.response.rect, sel_btn_rect)
}

pub(crate) fn transport_glyph(transport: Option<&str>) -> char {
    match transport {
        Some("usb") => glyph::USB,
        Some("bluetooth") => glyph::BLUETOOTH,
        Some("relay") => glyph::LINK,
        Some("wifi_direct") => glyph::WIFI_TETHERING,
        _ => glyph::WIFI,
    }
}
