use super::super::icons::{paint_refresh_spin, semibold};
use super::super::state::SharedUiCore;
use super::super::theme::ThemePalette;
use super::super::widgets::{
    paint_divider, paint_inline_capsule, small_accent_btn, switch_pill_btn,
};
use crate::updater::UpdatesOverviewState;
use egui::{Align, CornerRadius, FontId, Layout, Margin, Pos2, RichText, Sense, Stroke, Vec2};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) fn render_opt_updates(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        hdr,
        check_lbl,
        spin_active,
        upd,
        sync_ver,
        auto_app,
        auto_drv,
        auto_on_lbl,
        auto_off_lbl,
        portable_lbl,
        dl_lbl,
        inst_drv_lbl,
        dl_ing_lbl,
        restart_lbl,
        app_msg,
        drv_msg,
    ) = {
        let g = core.lock().unwrap();
        let u = g.updates.clone();
        let is_chk = u.app_status == "checking" || u.driver_status == "checking";
        let spin_on = is_chk || g.upd_spin_until.is_some_and(|t| Instant::now() < t);
        let app_m = translate_upd_msg(&g, true, &u);
        let drv_m = translate_upd_msg(&g, false, &u);
        (
            g.t("settings_updates_header").to_uppercase(),
            g.t(if is_chk {
                "settings_checking_updates"
            } else {
                "settings_check_updates"
            })
            .to_string(),
            spin_on,
            u,
            g.live.sync_version.clone(),
            g.live.ui_settings.auto_update_app,
            g.live.ui_settings.auto_update_driver,
            g.t("settings_auto_prefix_on").to_string(),
            g.t("settings_auto_prefix_off").to_string(),
            g.t("settings_mode_portable").to_string(),
            g.t("settings_btn_download_install").to_string(),
            g.t("settings_btn_install_driver").to_string(),
            g.t("settings_btn_downloading").to_string(),
            g.t("settings_btn_restart_now").to_string(),
            app_m,
            drv_m,
        )
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new(hdr).size(12.0).color(pal.on_surface_muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let gal = ui.painter().layout_no_wrap(
                check_lbl.clone(),
                FontId::proportional(11.0),
                pal.accent,
            );
            let w = gal.size().x + 20.0;
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::click());
            let ang = if spin_active {
                ui.ctx().request_repaint();
                (ui.input(|i| i.time) * 6.5) as f32
            } else {
                0.0
            };
            paint_refresh_spin(
                ui.painter(),
                Pos2::new(r.left() + 7.0, r.center().y),
                5.0,
                ang,
                pal.accent,
            );
            ui.painter().galley(
                Pos2::new(r.left() + 18.0, r.center().y - gal.size().y * 0.5),
                gal,
                pal.accent,
            );
            if resp.hovered() {
                ui.painter().line_segment(
                    [
                        Pos2::new(r.left() + 18.0, r.center().y + 7.0),
                        Pos2::new(r.right() - 2.0, r.center().y + 7.0),
                    ],
                    Stroke::new(1.0, pal.accent),
                );
            }
            if resp.clicked() {
                let mut g = core.lock().unwrap();
                g.upd_spin_until = Some(Instant::now() + Duration::from_millis(650));
                let u_bg = g.updater.clone();
                let dv = g.live.snapshot.driver_version.clone();
                let aa = g.live.ui_settings.auto_update_app;
                let ad = g.live.ui_settings.auto_update_driver;
                g.rt.spawn(async move {
                    u_bg.check_all(dv, aa, ad).await;
                });
            }
        });
    });

    ui.add_space(8.0);
    egui::Frame::new()
        .fill(pal.surface_elevated)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.horizontal(|ui| {
                let app_name = if upd.app_display_name.is_empty() {
                    "NFC Bridge Sync"
                } else {
                    &upd.app_display_name
                };
                ui.label(
                    RichText::new(app_name)
                        .font(semibold(14.0))
                        .color(pal.on_surface),
                );
                ui.add_space(6.0);
                let v_str = format!(
                    "v{}",
                    if upd.app_version.is_empty() {
                        &sync_ver
                    } else {
                        &upd.app_version
                    }
                );
                paint_inline_capsule(ui, &v_str, true, pal);
                if upd.is_portable {
                    ui.add_space(4.0);
                    paint_inline_capsule(ui, &portable_lbl, true, pal);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if switch_pill_btn(
                        ui,
                        pal,
                        if auto_app {
                            &auto_on_lbl
                        } else {
                            &auto_off_lbl
                        },
                        auto_app,
                    ) {
                        let mut g = core.lock().unwrap();
                        g.live.ui_settings.auto_update_app = !auto_app;
                        g.persist_ui_settings();
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(app_msg)
                    .size(11.0)
                    .color(if upd.app_status == "available" {
                        pal.accent
                    } else {
                        pal.on_surface_muted
                    }),
            );

            if upd.app_status == "available"
                || upd.app_status == "downloading"
                || upd.app_status == "installed_restart"
                || upd.pending_restart
            {
                ui.add_space(6.0);
                let lbl = if upd.app_status == "installed_restart" || upd.pending_restart {
                    &restart_lbl
                } else if upd.app_status == "downloading" {
                    &dl_ing_lbl
                } else {
                    &dl_lbl
                };
                if small_accent_btn(ui, pal, lbl, 160.0, 28.0) && upd.app_status != "downloading" {
                    let g = core.lock().unwrap();
                    let u_bg = g.updater.clone();
                    let is_res = upd.app_status == "installed_restart" || upd.pending_restart;
                    g.rt.spawn(async move {
                        if is_res {
                            let _ = u_bg.restart_running_app().await;
                        } else {
                            let _ = u_bg.install_app_update().await;
                        }
                    });
                }
            }

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("NFC Bridge Driver")
                        .font(semibold(14.0))
                        .color(pal.on_surface),
                );
                ui.add_space(6.0);
                let dv_str = upd
                    .driver_version
                    .as_deref()
                    .map(|v| format!("v{}", v))
                    .unwrap_or_else(|| "--".to_string());
                paint_inline_capsule(ui, &dv_str, true, pal);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if switch_pill_btn(
                        ui,
                        pal,
                        if auto_drv {
                            &auto_on_lbl
                        } else {
                            &auto_off_lbl
                        },
                        auto_drv,
                    ) {
                        let mut g = core.lock().unwrap();
                        g.live.ui_settings.auto_update_driver = !auto_drv;
                        g.persist_ui_settings();
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(drv_msg)
                    .size(11.0)
                    .color(if upd.driver_status == "available" {
                        pal.accent
                    } else {
                        pal.on_surface_muted
                    }),
            );

            if upd.driver_status == "available" || upd.driver_status == "downloading" {
                ui.add_space(6.0);
                let lbl = if upd.driver_status == "downloading" {
                    &dl_ing_lbl
                } else {
                    &inst_drv_lbl
                };
                if small_accent_btn(ui, pal, lbl, 148.0, 28.0) && upd.driver_status != "downloading"
                {
                    let g = core.lock().unwrap();
                    let u_bg = g.updater.clone();
                    g.rt.spawn(async move {
                        let _ = u_bg.install_driver_update().await;
                    });
                }
            }
        });
}

fn translate_upd_msg(g: &SharedUiCore, is_app: bool, u: &UpdatesOverviewState) -> String {
    let status = if is_app {
        &u.app_status
    } else {
        &u.driver_status
    };
    let avail = if is_app {
        &u.app_available
    } else {
        &u.driver_available
    };
    let cur = if is_app {
        Some(if u.app_version.is_empty() {
            g.live.sync_version.clone()
        } else {
            u.app_version.clone()
        })
    } else {
        u.driver_version.clone()
    };
    match status.as_str() {
        "checking" => g.t("upd_checking").to_string(),
        "up_to_date" => cur
            .map(|v| g.t_vars("upd_uptodate", &[("ver", &v)]))
            .unwrap_or_else(|| g.t("upd_idle").to_string()),
        "available" => {
            if let Some(a) = avail {
                g.t_vars(
                    if is_app {
                        "upd_available_short"
                    } else {
                        "upd_driver_available"
                    },
                    &[("ver", &a.version), ("asset", &a.asset_name)],
                )
            } else {
                g.t("upd_idle").to_string()
            }
        }
        "downloading" => {
            let asset = avail.as_ref().map(|a| a.asset_name.as_str()).unwrap_or("");
            g.t_vars("upd_downloading", &[("asset", asset)])
        }
        "installed_restart" => avail
            .as_ref()
            .map(|a| g.t_vars("upd_restart_ready", &[("ver", &a.version)]))
            .unwrap_or_else(|| g.t("upd_idle").to_string()),
        "no_releases" => {
            if is_app {
                let n = if u.app_display_name.is_empty() {
                    "NFC Bridge Sync"
                } else {
                    &u.app_display_name
                };
                g.t_vars("upd_no_releases", &[("name", n)])
            } else {
                g.t("upd_no_releases_driver").to_string()
            }
        }
        _ => g.t("upd_idle").to_string(),
    }
}
