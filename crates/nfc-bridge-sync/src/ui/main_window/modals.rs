use super::super::icons::{glyph, icon_rich};
use super::super::state::SharedUiCore;
use super::super::theme::ThemePalette;
use super::super::widgets::{small_accent_btn, small_danger_btn, small_secondary_btn};
use crate::ipc::IpcCommand;
use egui::{Align, Color32, CornerRadius, Id, Layout, Margin, Rect, RichText, Stroke, Vec2};
use std::sync::{Arc, Mutex};

pub(crate) fn render_body_modals(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    body_rect: Rect,
) {
    #[derive(Clone)]
    enum ActiveModal {
        MobileSwitch {
            req_id: String,
            title: String,
            desc: String,
            reject: String,
            accept: String,
        },
        InvalidPairing {
            dev_id: Option<String>,
            title: String,
            desc: String,
            keep: String,
            delete: String,
        },
        RestartReady {
            title: String,
            desc: String,
            later: String,
            now_lbl: String,
        },
    }

    let modal = {
        let g = core.lock().unwrap();
        let app_name = g.app_display_name();
        if let Some(ref sw) = g.live.pending_mobile_switch {
            let tr_lbl = match sw.transport.as_str() {
                "tcp" => g.t("qr_method_tcp"),
                "bluetooth" => g.t("qr_method_bluetooth"),
                "wifi_direct" => g.t("qr_method_wifi_direct"),
                "usb" => g.t("method_usb_title"),
                _ => g.t("qr_method_relay"),
            };
            Some(ActiveModal::MobileSwitch {
                req_id: sw.request_id.clone(),
                title: g.t("modal_switch_title").to_string(),
                desc: g.t_vars(
                    "modal_switch_desc",
                    &[
                        ("name", &sw.device_name),
                        ("transport", tr_lbl),
                        ("app", &app_name),
                    ],
                ),
                reject: g.t("modal_switch_reject").to_string(),
                accept: g.t("modal_switch_accept").to_string(),
            })
        } else if let Some(ref inv) = g.live.pending_invalid_pairing {
            Some(ActiveModal::InvalidPairing {
                dev_id: inv.device_id.clone(),
                title: g.t("modal_invalid_title").to_string(),
                desc: g.t_vars("modal_invalid_desc", &[("name", &inv.device_name)]),
                keep: g.t("modal_invalid_keep").to_string(),
                delete: g.t("modal_invalid_delete").to_string(),
            })
        } else if g.updates.pending_restart && !g.restart_dismissed {
            Some(ActiveModal::RestartReady {
                title: g.t("modal_restart_title").to_string(),
                desc: g.t("modal_restart_desc").to_string(),
                later: g.t("modal_restart_later").to_string(),
                now_lbl: g.t("modal_restart_now").to_string(),
            })
        } else {
            None
        }
    };

    let Some(m) = modal else { return };

    egui::Area::new(Id::unique("nb_body_modal_scrim"))
        .order(egui::Order::Foreground)
        .fixed_pos(body_rect.min)
        .show(ui.ctx(), |ui| {
            ui.painter().rect_filled(
                body_rect,
                CornerRadius::ZERO,
                Color32::from_black_alpha(158),
            );
            let card_rect = Rect::from_center_size(body_rect.center(), Vec2::new(390.0, 168.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(card_rect), |ui| {
                egui::Frame::new()
                    .fill(pal.surface_card)
                    .stroke(Stroke::new(1.0, pal.outline))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_min_size(Vec2::new(358.0, 136.0));
                        match m {
                            ActiveModal::MobileSwitch {
                                req_id,
                                title,
                                desc,
                                reject,
                                accept,
                            } => {
                                ui.horizontal(|ui| {
                                    ui.label(icon_rich(glyph::SWAP_HORIZ, 18.0, pal.accent));
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(title)
                                            .size(15.0)
                                            .strong()
                                            .color(pal.on_surface),
                                    );
                                });
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(desc).size(12.5).color(pal.on_surface_muted),
                                );
                                ui.add_space(16.0);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if small_accent_btn(ui, pal, &accept, 120.0, 32.0) {
                                        let g = core.lock().unwrap();
                                        let ipc = g.ipc.clone();
                                        let rid = req_id.clone();
                                        g.rt.spawn(async move {
                                            ipc.clear_pending_mobile_switch(Some(&rid)).await;
                                            let _ = ipc
                                                .send_command(IpcCommand::RespondMobileSwitch {
                                                    request_id: rid,
                                                    accept: true,
                                                })
                                                .await;
                                        });
                                    }
                                    ui.add_space(8.0);
                                    if small_secondary_btn(ui, pal, &reject, 96.0, 32.0) {
                                        let g = core.lock().unwrap();
                                        let ipc = g.ipc.clone();
                                        let rid = req_id.clone();
                                        g.rt.spawn(async move {
                                            ipc.clear_pending_mobile_switch(Some(&rid)).await;
                                            let _ = ipc
                                                .send_command(IpcCommand::RespondMobileSwitch {
                                                    request_id: rid,
                                                    accept: false,
                                                })
                                                .await;
                                        });
                                    }
                                });
                            }
                            ActiveModal::InvalidPairing {
                                dev_id,
                                title,
                                desc,
                                keep,
                                delete,
                            } => {
                                ui.horizontal(|ui| {
                                    ui.label(icon_rich(glyph::WARNING, 18.0, pal.red_disconnect));
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(title)
                                            .size(15.0)
                                            .strong()
                                            .color(pal.on_surface),
                                    );
                                });
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(desc).size(12.5).color(pal.on_surface_muted),
                                );
                                ui.add_space(16.0);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if small_danger_btn(ui, pal, &delete, 118.0, 32.0) {
                                        let g = core.lock().unwrap();
                                        let ipc = g.ipc.clone();
                                        g.rt.spawn(async move {
                                            ipc.clear_invalid_pairing_notice().await;
                                            if let Some(id) = dev_id {
                                                ipc.remove_saved_profile(&id).await;
                                                let _ = ipc
                                                    .send_command(IpcCommand::RevokeTrustedDevice {
                                                        device_id: id,
                                                    })
                                                    .await;
                                            }
                                        });
                                    }
                                    ui.add_space(8.0);
                                    if small_secondary_btn(ui, pal, &keep, 96.0, 32.0) {
                                        let g = core.lock().unwrap();
                                        let ipc = g.ipc.clone();
                                        g.rt.spawn(async move {
                                            ipc.clear_invalid_pairing_notice().await;
                                        });
                                    }
                                });
                            }
                            ActiveModal::RestartReady {
                                title,
                                desc,
                                later,
                                now_lbl,
                            } => {
                                ui.horizontal(|ui| {
                                    ui.label(icon_rich(
                                        glyph::SYSTEM_UPDATE_ALT,
                                        18.0,
                                        pal.accent,
                                    ));
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(title)
                                            .size(15.0)
                                            .strong()
                                            .color(pal.on_surface),
                                    );
                                });
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(desc).size(12.5).color(pal.on_surface_muted),
                                );
                                ui.add_space(16.0);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if small_accent_btn(ui, pal, &now_lbl, 118.0, 32.0) {
                                        let g = core.lock().unwrap();
                                        let upd = g.updater.clone();
                                        g.rt.spawn(async move {
                                            let _ = upd.restart_running_app().await;
                                        });
                                    }
                                    ui.add_space(8.0);
                                    if small_secondary_btn(ui, pal, &later, 96.0, 32.0) {
                                        core.lock().unwrap().restart_dismissed = true;
                                    }
                                });
                            }
                        }
                    });
            });
        });
}
