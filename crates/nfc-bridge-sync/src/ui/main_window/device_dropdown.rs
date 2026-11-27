use super::super::icons::{glyph, paint_device_os_icon, paint_glyph, semibold};
use super::super::state::SharedUiCore;
use super::super::theme::ThemePalette;
use super::super::widgets::{styled_singleline_input, truncate_text_to_width};
use super::left_column::transport_glyph;
use crate::ipc::IpcCommand;
use egui::{
    Color32, CornerRadius, FontId, Id, Margin, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use std::sync::{Arc, Mutex};

pub(crate) fn render_device_dropdown_overlay(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    card_anchor_rect: Rect,
    sel_btn_rect: Rect,
) {
    let (
        devices,
        active_id,
        selected_id,
        connected_now,
        paused_now,
        latency_now,
        editing_id,
        mut editing_name,
        ph_lbl,
        short_disconn,
        short_paused,
    ) = {
        let mut g = core.lock().unwrap();
        if !g.device_dropdown_open {
            return;
        }
        if g.live.snapshot.trusted_devices.is_empty() {
            g.device_dropdown_open = false;
            g.editing_device_id = None;
            return;
        }
        (
            g.live.snapshot.trusted_devices.clone(),
            g.live.active_device_id.clone(),
            g.live.selected_device_id.clone(),
            g.live.connected,
            g.live.paused,
            g.live.latency_ms,
            g.editing_device_id.clone(),
            g.editing_device_name.clone(),
            g.t("device_name_placeholder").to_string(),
            g.t("device_disconnected_short").to_string(),
            g.t("device_paused_short").to_string(),
        )
    };

    if !card_anchor_rect.is_positive() {
        return;
    }

    let inner_w = (card_anchor_rect.width() - 14.0).max(220.0);
    let dd_pos = Pos2::new(card_anchor_rect.left(), card_anchor_rect.bottom() + 4.0);
    let row_count = devices.len().max(1);
    let row_h = 48.0;
    let row_gap = 4.0;
    let max_visible_h = 4.5 * row_h + 4.0 * row_gap;
    let target_h = (row_count as f32 * row_h + (row_count.saturating_sub(1)) as f32 * row_gap)
        .min(max_visible_h);

    let area_resp = egui::Area::new(Id::unique("nb_device_dropdown_area"))
        .order(egui::Order::Foreground)
        .fixed_pos(dd_pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(pal.surface_card)
                .stroke(Stroke::new(1.0, pal.outline))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(6))
                .shadow(egui::Shadow {
                    offset: [0, 8],
                    blur: 20,
                    spread: 0,
                    color: Color32::from_black_alpha(140),
                })
                .show(ui, |ui| {
                    ui.set_min_width(inner_w);
                    ui.set_max_width(inner_w);
                    egui::ScrollArea::vertical()
                        .id_salt("nb_device_dd_scroll")
                        .max_width(inner_w)
                        .min_scrolled_height(target_h)
                        .max_height(target_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_width(inner_w);
                            ui.set_max_width(inner_w);
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            for (idx, dev) in devices.iter().enumerate() {
                                let is_active =
                                    connected_now && Some(&dev.device_id) == active_id.as_ref();
                                let is_selected_saved = !connected_now
                                    && (Some(&dev.device_id) == selected_id.as_ref()
                                        || (selected_id.is_none() && idx == 0));
                                let is_editing =
                                    editing_id.as_deref() == Some(dev.device_id.as_str());
                                let show_check = is_active || is_selected_saved;

                                let (row_outer_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(inner_w, 48.0),
                                    Sense::hover(),
                                );
                                let pointer_over_row = is_editing
                                    || ui
                                        .ctx()
                                        .pointer_hover_pos()
                                        .is_some_and(|p| row_outer_rect.contains(p));

                                let (row_bg, row_border) = if is_active {
                                    (pal.accent_card_bg, pal.accent_border)
                                } else if is_selected_saved {
                                    (pal.surface_elevated, pal.outline_variant)
                                } else if pointer_over_row {
                                    (pal.surface_elevated, Color32::TRANSPARENT)
                                } else {
                                    (Color32::TRANSPARENT, Color32::TRANSPARENT)
                                };

                                ui.painter().rect(
                                    row_outer_rect,
                                    CornerRadius::same(8),
                                    row_bg,
                                    Stroke::new(1.0, row_border),
                                    StrokeKind::Inside,
                                );

                                let content_r = row_outer_rect.shrink2(Vec2::new(10.0, 8.0));
                                let dot_c = if is_active {
                                    if paused_now {
                                        pal.amber_pause
                                    } else {
                                        pal.led_green
                                    }
                                } else {
                                    pal.red_disconnect
                                };
                                ui.painter().circle_filled(
                                    Pos2::new(content_r.left() + 4.0, content_r.center().y),
                                    4.0,
                                    dot_c,
                                );

                                let tr_box = Rect::from_min_size(
                                    Pos2::new(content_r.left() + 18.0, content_r.top()),
                                    Vec2::splat(32.0),
                                );
                                let (tr_bg, tr_bd, tr_fg) = if is_active {
                                    (pal.accent_soft, pal.accent_border, pal.accent)
                                } else {
                                    (pal.surface_elevated, pal.outline, pal.on_surface_muted)
                                };
                                ui.painter().rect(
                                    tr_box,
                                    CornerRadius::same(8),
                                    tr_bg,
                                    Stroke::new(1.0, tr_bd),
                                    StrokeKind::Inside,
                                );
                                paint_glyph(
                                    ui.painter(),
                                    tr_box.center(),
                                    transport_glyph(dev.transport.as_deref()),
                                    18.0,
                                    tr_fg,
                                );

                                let dl_r = Rect::from_min_size(
                                    Pos2::new(content_r.right() - 28.0, content_r.center().y - 14.0),
                                    Vec2::splat(28.0),
                                );
                                let ed_r = Rect::from_min_size(
                                    Pos2::new(dl_r.left() - 32.0, content_r.center().y - 14.0),
                                    Vec2::splat(28.0),
                                );
                                let chk_center_x = if pointer_over_row {
                                    ed_r.left() - 14.0
                                } else {
                                    content_r.right() - 10.0
                                };

                                if is_editing {
                                    let edit_left = tr_box.right() + 10.0;
                                    let edit_w = (ed_r.left() - 40.0 - edit_left).max(70.0);
                                    let edit_y = content_r.center().y - 14.0;
                                    let edit_rect = Rect::from_min_size(
                                        Pos2::new(edit_left, edit_y),
                                        Vec2::new(edit_w, 28.0),
                                    );
                                    let mut enter_pressed = false;
                                    ui.scope_builder(
                                        egui::UiBuilder::new().max_rect(edit_rect),
                                        |ui| {
                                            let te = styled_singleline_input(
                                                ui,
                                                pal,
                                                &mut editing_name,
                                                edit_w,
                                                28.0,
                                                &ph_lbl,
                                            );
                                            enter_pressed = te.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                        },
                                    );
                                    core.lock().unwrap().editing_device_name = editing_name.clone();

                                    let save_r = Rect::from_min_size(
                                        Pos2::new(edit_rect.right() + 4.0, edit_y),
                                        Vec2::splat(28.0),
                                    );
                                    let save_resp = ui.interact(
                                        save_r,
                                        Id::unique(("dd_save", &dev.device_id)),
                                        Sense::click(),
                                    );
                                    ui.painter().rect_filled(
                                        save_r,
                                        CornerRadius::same(8),
                                        pal.accent,
                                    );
                                    paint_glyph(
                                        ui.painter(),
                                        save_r.center(),
                                        glyph::CHECK,
                                        16.0,
                                        pal.on_accent,
                                    );
                                    if save_resp.clicked() || enter_pressed {
                                        let clean = editing_name.trim().to_string();
                                        let mut g = core.lock().unwrap();
                                        if !clean.is_empty() {
                                            let id = dev.device_id.clone();
                                            let ipc = g.ipc.clone();
                                            g.rt.spawn(async move {
                                                ipc.rename_saved_profile(&id, &clean).await;
                                                let _ = ipc
                                                    .send_command(IpcCommand::RenameTrustedDevice {
                                                        device_id: id,
                                                        name: clean,
                                                    })
                                                    .await;
                                            });
                                        }
                                        g.editing_device_id = None;
                                    }
                                } else {
                                    let text_left = tr_box.right() + 10.0;
                                    let right_limit = if show_check {
                                        chk_center_x - 14.0
                                    } else if pointer_over_row {
                                        ed_r.left() - 6.0
                                    } else {
                                        content_r.right() - 4.0
                                    };
                                    let click_rect = Rect::from_min_max(
                                        row_outer_rect.min,
                                        Pos2::new(
                                            if pointer_over_row {
                                                ed_r.left() - 4.0
                                            } else {
                                                row_outer_rect.right()
                                            },
                                            row_outer_rect.bottom(),
                                        ),
                                    );
                                    let row_resp = ui.interact(
                                        click_rect,
                                        Id::unique(("dd_row", &dev.device_id)),
                                        Sense::click(),
                                    );

                                    let os_r = Rect::from_min_size(
                                        Pos2::new(text_left, content_r.top() + 2.0),
                                        Vec2::splat(14.0),
                                    );
                                    let os_col = if is_active {
                                        pal.on_surface
                                    } else {
                                        pal.on_surface_muted
                                    };
                                    paint_device_os_icon(
                                        ui.painter(),
                                        os_r,
                                        dev.device_os.as_deref().unwrap_or("android"),
                                        os_col,
                                    );

                                    let max_name_w = (right_limit - (text_left + 20.0)).max(50.0);
                                    let trunc_name = truncate_text_to_width(
                                        ui.painter(),
                                        &dev.device_name,
                                        semibold(14.0),
                                        max_name_w,
                                    );
                                    ui.painter().text(
                                        Pos2::new(text_left + 20.0, content_r.top()),
                                        egui::Align2::LEFT_TOP,
                                        trunc_name,
                                        semibold(14.0),
                                        pal.on_surface,
                                    );

                                    let raw_id = dev
                                        .mobile_id
                                        .as_deref()
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or(&dev.device_id);
                                    let st_part = if is_active {
                                        if paused_now {
                                            short_paused.as_str().to_string()
                                        } else {
                                            format!("{} ms", latency_now.max(1))
                                        }
                                    } else {
                                        short_disconn.clone()
                                    };
                                    let sub_txt = format!("ID {} • {}", raw_id, st_part);
                                    let trunc_sub = truncate_text_to_width(
                                        ui.painter(),
                                        &sub_txt,
                                        FontId::monospace(10.0),
                                        (right_limit - text_left).max(50.0),
                                    );
                                    ui.painter().text(
                                        Pos2::new(text_left, content_r.top() + 18.0),
                                        egui::Align2::LEFT_TOP,
                                        trunc_sub,
                                        FontId::monospace(10.0),
                                        pal.on_surface_muted,
                                    );

                                    if row_resp.clicked() {
                                        let id = dev.device_id.clone();
                                        let mut g = core.lock().unwrap();
                                        g.device_dropdown_open = false;
                                        let ipc = g.ipc.clone();
                                        g.rt.spawn(async move {
                                            ipc.select_saved_profile(&id).await;
                                            let _ = ipc
                                                .send_command(IpcCommand::SelectTrustedDevice {
                                                    device_id: id,
                                                })
                                                .await;
                                        });
                                    }

                                    if show_check {
                                        paint_glyph(
                                            ui.painter(),
                                            Pos2::new(chk_center_x, content_r.center().y),
                                            glyph::CHECK,
                                            16.0,
                                            if is_active {
                                                pal.accent
                                            } else {
                                                pal.on_surface_muted
                                            },
                                        );
                                    }
                                }

                                if pointer_over_row {
                                    let ed_resp = ui.interact(
                                        ed_r,
                                        Id::unique(("dd_edit", &dev.device_id)),
                                        Sense::click(),
                                    );
                                    ui.painter().rect(
                                        ed_r,
                                        CornerRadius::same(8),
                                        Color32::TRANSPARENT,
                                        Stroke::new(
                                            1.0,
                                            if ed_resp.hovered() {
                                                pal.accent
                                            } else {
                                                pal.outline
                                            },
                                        ),
                                        StrokeKind::Inside,
                                    );
                                    paint_glyph(
                                        ui.painter(),
                                        ed_r.center(),
                                        glyph::EDIT,
                                        15.0,
                                        if ed_resp.hovered() {
                                            pal.accent
                                        } else {
                                            pal.on_surface_muted
                                        },
                                    );
                                    if ed_resp.clicked() {
                                        let mut g = core.lock().unwrap();
                                        g.editing_device_id = if is_editing {
                                            None
                                        } else {
                                            Some(dev.device_id.clone())
                                        };
                                        g.editing_device_name = dev.device_name.clone();
                                    }

                                    let dl_resp = ui.interact(
                                        dl_r,
                                        Id::unique(("dd_del", &dev.device_id)),
                                        Sense::click(),
                                    );
                                    ui.painter().rect(
                                        dl_r,
                                        CornerRadius::same(8),
                                        if dl_resp.hovered() {
                                            pal.red_soft
                                        } else {
                                            Color32::TRANSPARENT
                                        },
                                        Stroke::new(
                                            1.0,
                                            if dl_resp.hovered() {
                                                pal.red_border
                                            } else {
                                                pal.outline
                                            },
                                        ),
                                        StrokeKind::Inside,
                                    );
                                    paint_glyph(
                                        ui.painter(),
                                        dl_r.center(),
                                        glyph::DELETE,
                                        15.0,
                                        if dl_resp.hovered() {
                                            pal.red_disconnect
                                        } else {
                                            pal.on_surface_muted
                                        },
                                    );
                                    if dl_resp.clicked() {
                                        let id = dev.device_id.clone();
                                        let mut g = core.lock().unwrap();
                                        g.live.snapshot.trusted_devices.retain(|d| d.device_id != id);
                                        if g.live.snapshot.trusted_devices.is_empty() {
                                            g.device_dropdown_open = false;
                                            g.editing_device_id = None;
                                        } else if g.editing_device_id.as_deref() == Some(id.as_str()) {
                                            g.editing_device_id = None;
                                        }
                                        g.snapshot_dirty_until = Some(
                                            std::time::Instant::now()
                                                + std::time::Duration::from_millis(900),
                                        );
                                        let ipc = g.ipc.clone();
                                        g.rt.spawn(async move {
                                            ipc.remove_saved_profile(&id).await;
                                            let _ = ipc
                                                .send_command(IpcCommand::RevokeTrustedDevice {
                                                    device_id: id,
                                                })
                                                .await;
                                        });
                                    }
                                }

                                if idx < devices.len() - 1 {
                                    ui.add_space(4.0);
                                }
                            }
                        });
                });
        });

    let esc_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let click_outside = ui.input(|i| i.pointer.any_pressed())
        && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
            !area_resp.response.rect.contains(p) && !sel_btn_rect.contains(p)
        });
    if esc_pressed || click_outside {
        let mut g = core.lock().unwrap();
        g.device_dropdown_open = false;
        g.editing_device_id = None;
    }
}
