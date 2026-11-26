mod bluetooth;
mod qr;
mod relay;
mod tcp;
mod usb;
mod wifi_direct;

pub(crate) use qr::{render_qr_panel, trigger_qr_request};

use super::super::icons::semibold;
use super::super::state::SharedUiCore;
use super::super::theme::ThemePalette;
use super::super::widgets::paint_status_badge;
use egui::{Align, Layout, RichText};
use std::sync::{Arc, Mutex};

pub(crate) fn render_transport_panel(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    panel: &str,
) {
    match panel {
        "usb" => usb::render_panel_usb(core, ui, pal),
        "tcp" => tcp::render_panel_tcp(core, ui, pal),
        "bluetooth" => bluetooth::render_panel_bt(core, ui, pal),
        "relay" => relay::render_panel_relay(core, ui, pal),
        "wifi_direct" => wifi_direct::render_panel_wfd(core, ui, pal),
        _ => usb::render_panel_usb(core, ui, pal),
    }
}

pub(crate) fn panel_header(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    title: &str,
    status_txt: &str,
    status_on: bool,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(16.0)).color(pal.on_surface));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            paint_status_badge(ui, status_txt, status_on, pal);
        });
    });
}

pub(crate) fn panel_push_footer(ui: &mut egui::Ui, footer_h: f32) {
    let rem = ui.available_height();
    if rem > footer_h {
        ui.add_space(rem - footer_h);
    }
}
