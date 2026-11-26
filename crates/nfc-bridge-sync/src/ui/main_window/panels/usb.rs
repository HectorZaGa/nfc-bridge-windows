use super::super::super::icons::{glyph, paint_glyph, paint_two_line_centered, semibold};
use super::super::super::state::SharedUiCore;
use super::super::super::theme::ThemePalette;
use super::super::super::widgets::{primary_cta_btn, toggle_state_btn};
use super::{panel_header, panel_push_footer};
use crate::ipc::IpcCommand;
use egui::{CornerRadius, FontId, Margin, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::sync::{Arc, Mutex};

pub(crate) fn render_panel_usb(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (
        title,
        status_txt,
        status_on,
        hero_title,
        hero_sub,
        auto_desc,
        auto_lbl,
        auto_on_lbl,
        usb_auto,
        usb_enabled,
        toggle_lbl,
    ) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let usb_en = snap.usb_enabled;
        let st_txt = if snap.usb_connected {
            g.t("panel_usb_status_connected")
        } else if usb_en {
            g.t("panel_usb_status_listening")
        } else {
            g.t("panel_usb_status_disabled")
        };
        (
            g.t("method_usb_title").to_string(),
            st_txt.to_string(),
            usb_en || snap.usb_connected,
            g.t(if usb_en {
                "panel_usb_native_active"
            } else {
                "panel_usb_native_off"
            })
            .to_string(),
            g.t("panel_usb_subhint").to_string(),
            g.t("panel_usb_auto_desc").to_string(),
            g.t("panel_usb_auto_label").to_string(),
            g.t(if snap.usb_auto_connect {
                "state_on"
            } else {
                "state_off"
            })
            .to_string(),
            snap.usb_auto_connect,
            usb_en,
            g.t(if usb_en {
                "panel_usb_listen_on"
            } else {
                "panel_usb_listen_off"
            })
            .to_string(),
        )
    };

    panel_header(ui, pal, &title, &status_txt, status_on);

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(120.0);
    ui.add_space(((body_h - 114.0) * 0.5).max(12.0));

    egui::Frame::new()
        .fill(pal.surface_elevated)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            let (row_r, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
            let ic_r = Rect::from_min_size(row_r.min, Vec2::splat(40.0));
            ui.painter().rect(
                ic_r,
                CornerRadius::same(8),
                pal.accent_soft,
                Stroke::new(1.0, pal.accent_border),
                StrokeKind::Inside,
            );
            paint_glyph(ui.painter(), ic_r.center(), glyph::USB, 18.0, pal.accent);
            paint_two_line_centered(
                ui.painter(),
                ic_r.right() + 12.0,
                ic_r.center().y,
                &hero_title,
                semibold(14.0),
                pal.on_surface,
                &hero_sub,
                FontId::monospace(11.0),
                pal.on_surface_muted,
                6.5,
            );
        });

    ui.add_space(12.0);
    ui.label(
        RichText::new(auto_desc)
            .size(11.0)
            .color(pal.on_surface_muted),
    );

    panel_push_footer(ui, footer_h);

    if toggle_state_btn(
        ui,
        pal,
        if usb_auto {
            glyph::USB
        } else {
            glyph::USB_OFF
        },
        &auto_lbl,
        &auto_on_lbl,
        usb_auto,
    ) {
        core.lock()
            .unwrap()
            .send_ipc(IpcCommand::SetUsbPriority { enabled: !usb_auto });
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::CABLE, &toggle_lbl, usb_enabled) {
        core.lock()
            .unwrap()
            .send_ipc(IpcCommand::SetUsbEnabled {
                enabled: !usb_enabled,
            });
    }
}
