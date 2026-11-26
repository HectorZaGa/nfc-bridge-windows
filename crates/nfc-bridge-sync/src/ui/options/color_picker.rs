use super::super::state::SharedUiCore;
use super::super::theme::{hsv_to_rgb, parse_hex_rgb, rgb_to_hex, rgb_to_hsv, ThemePalette};
use super::super::widgets::{paint_status_badge, small_accent_btn, small_secondary_btn};
use egui::{
    Align, Color32, CornerRadius, FontId, Id, Layout, Margin, Mesh, Pos2, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use std::sync::{Arc, Mutex};

pub(crate) fn render_hsv_color_popover(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
) {
    let (anchor, mut h, mut s, mut v, mut hex_buf, recents, cp_title, cp_reset, cp_good, cp_low, cp_recent, cp_cancel, cp_apply) = {
        let g = core.lock().unwrap();
        if !g.color_picker_open {
            return;
        }
        (
            g.color_picker_anchor,
            g.hsv_h,
            g.hsv_s,
            g.hsv_v,
            g.hsv_hex_buf.clone(),
            g.recent_accents.clone(),
            g.t("cp_title").to_string(),
            g.t("cp_reset").to_string(),
            g.t("cp_good_contrast").to_string(),
            g.t("cp_low_contrast").to_string(),
            g.t("cp_recent").to_string(),
            g.t("cp_cancel").to_string(),
            g.t("cp_apply").to_string(),
        )
    };

    let pos = Pos2::new(
        (anchor.x - 280.0).clamp(16.0, 354.0),
        (anchor.y + 8.0).clamp(52.0, 140.0),
    );
    egui::Area::new(Id::unique("nb_hsv_popover"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(pal.surface_card)
                .stroke(Stroke::new(1.0, pal.outline_variant))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(282.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(cp_title)
                                .size(13.5)
                                .strong()
                                .color(pal.on_surface),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .link(RichText::new(cp_reset).size(11.5).color(pal.accent))
                                .clicked()
                            {
                                h = 217.0;
                                s = 0.76;
                                v = 0.96;
                                let (r, gr, b) = hsv_to_rgb(h, s, v);
                                hex_buf = rgb_to_hex(r, gr, b);
                            }
                        });
                    });
                    ui.add_space(10.0);

                    let (sv_rect, sv_resp) =
                        ui.allocate_exact_size(Vec2::new(282.0, 108.0), Sense::click_and_drag());
                    let (hr, hg, hb) = hsv_to_rgb(h, 1.0, 1.0);
                    let pure_hue = Color32::from_rgb(hr, hg, hb);
                    let mut mesh = Mesh::default();
                    let i0 = mesh.vertices.len() as u32;
                    mesh.colored_vertex(sv_rect.left_top(), Color32::WHITE);
                    mesh.colored_vertex(sv_rect.right_top(), pure_hue);
                    mesh.colored_vertex(sv_rect.right_bottom(), Color32::BLACK);
                    mesh.colored_vertex(sv_rect.left_bottom(), Color32::BLACK);
                    mesh.add_triangle(i0, i0 + 1, i0 + 2);
                    mesh.add_triangle(i0, i0 + 2, i0 + 3);
                    ui.painter().add(mesh);
                    ui.painter().rect_stroke(
                        sv_rect,
                        CornerRadius::same(8),
                        Stroke::new(1.0, pal.outline),
                        StrokeKind::Inside,
                    );

                    if (sv_resp.clicked() || sv_resp.dragged())
                        && sv_resp.interact_pointer_pos().is_some()
                    {
                        let p = sv_resp.interact_pointer_pos().unwrap();
                        s = ((p.x - sv_rect.left()) / sv_rect.width()).clamp(0.0, 1.0);
                        v = 1.0 - ((p.y - sv_rect.top()) / sv_rect.height()).clamp(0.0, 1.0);
                        let (r, gr, b) = hsv_to_rgb(h, s, v);
                        hex_buf = rgb_to_hex(r, gr, b);
                    }
                    let thumb = Pos2::new(
                        sv_rect.left() + s * sv_rect.width(),
                        sv_rect.top() + (1.0 - v) * sv_rect.height(),
                    );
                    let (cr, cg, cb) = hsv_to_rgb(h, s, v);
                    ui.painter().circle(
                        thumb,
                        6.5,
                        Color32::from_rgb(cr, cg, cb),
                        Stroke::new(2.0, Color32::WHITE),
                    );

                    ui.add_space(10.0);

                    let (hue_rect, hue_resp) =
                        ui.allocate_exact_size(Vec2::new(282.0, 14.0), Sense::click_and_drag());
                    let stops = [
                        (0.0, Color32::from_rgb(255, 0, 0)),
                        (1.0 / 6.0, Color32::from_rgb(255, 255, 0)),
                        (2.0 / 6.0, Color32::from_rgb(0, 255, 0)),
                        (3.0 / 6.0, Color32::from_rgb(0, 255, 255)),
                        (4.0 / 6.0, Color32::from_rgb(0, 0, 255)),
                        (5.0 / 6.0, Color32::from_rgb(255, 0, 255)),
                        (1.0, Color32::from_rgb(255, 0, 0)),
                    ];
                    let mut hmesh = Mesh::default();
                    for idx in 0..stops.len() - 1 {
                        let x0 = hue_rect.left() + stops[idx].0 * hue_rect.width();
                        let x1 = hue_rect.left() + stops[idx + 1].0 * hue_rect.width();
                        let base = hmesh.vertices.len() as u32;
                        hmesh.colored_vertex(Pos2::new(x0, hue_rect.top()), stops[idx].1);
                        hmesh.colored_vertex(Pos2::new(x1, hue_rect.top()), stops[idx + 1].1);
                        hmesh.colored_vertex(Pos2::new(x1, hue_rect.bottom()), stops[idx + 1].1);
                        hmesh.colored_vertex(Pos2::new(x0, hue_rect.bottom()), stops[idx].1);
                        hmesh.add_triangle(base, base + 1, base + 2);
                        hmesh.add_triangle(base, base + 2, base + 3);
                    }
                    ui.painter().add(hmesh);
                    ui.painter().rect_stroke(
                        hue_rect,
                        CornerRadius::same(7),
                        Stroke::new(1.0, pal.outline),
                        StrokeKind::Inside,
                    );

                    if (hue_resp.clicked() || hue_resp.dragged())
                        && hue_resp.interact_pointer_pos().is_some()
                    {
                        let p = hue_resp.interact_pointer_pos().unwrap();
                        h = (((p.x - hue_rect.left()) / hue_rect.width()).clamp(0.0, 1.0)) * 360.0;
                        let (r, gr, b) = hsv_to_rgb(h, s, v);
                        hex_buf = rgb_to_hex(r, gr, b);
                    }
                    let hthumb = Pos2::new(
                        hue_rect.left() + (h / 360.0) * hue_rect.width(),
                        hue_rect.center().y,
                    );
                    ui.painter().circle(
                        hthumb,
                        7.5,
                        pure_hue,
                        Stroke::new(2.0, Color32::WHITE),
                    );

                    ui.add_space(10.0);

                    let (cr, cg, cb) = hsv_to_rgb(h, s, v);
                    let lum = (0.299 * cr as f32 + 0.587 * cg as f32 + 0.114 * cb as f32) / 255.0;
                    egui::Frame::new()
                        .fill(pal.surface_elevated)
                        .stroke(Stroke::new(1.0, pal.outline))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (pr, _) =
                                    ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                                ui.painter().circle_filled(
                                    pr.center(),
                                    13.0,
                                    Color32::from_rgb(cr, cg, cb),
                                );
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    let hex_resp = ui.add_sized(
                                        [74.0, 20.0],
                                        egui::TextEdit::singleline(&mut hex_buf)
                                            .font(FontId::monospace(11.5)),
                                    );
                                    if hex_resp.changed()
                                        && hex_buf.trim().trim_start_matches('#').len() == 6
                                    {
                                        let (nr, ng, nb) = parse_hex_rgb(&hex_buf);
                                        let (nh, ns, nv) = rgb_to_hsv(nr, ng, nb);
                                        h = nh;
                                        s = ns;
                                        v = nv;
                                    }
                                    ui.label(
                                        RichText::new(format!("RGB({}, {}, {})", cr, cg, cb))
                                            .monospace()
                                            .size(10.0)
                                            .color(pal.on_surface_muted),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let good = (0.28..=0.85).contains(&lum);
                                    paint_status_badge(
                                        ui,
                                        if good { &cp_good } else { &cp_low },
                                        good,
                                        pal,
                                    );
                                });
                            });
                        });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(cp_recent)
                                .size(11.0)
                                .color(pal.on_surface_muted),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            for rhex in recents.iter().rev() {
                                let (rr, rg, rb) = parse_hex_rgb(rhex);
                                let (r_rect, r_resp) =
                                    ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                                ui.painter().circle(
                                    r_rect.center(),
                                    9.0,
                                    Color32::from_rgb(rr, rg, rb),
                                    Stroke::new(1.0, Color32::from_white_alpha(64)),
                                );
                                if r_resp.clicked() {
                                    let (nh, ns, nv) = rgb_to_hsv(rr, rg, rb);
                                    h = nh;
                                    s = ns;
                                    v = nv;
                                    hex_buf = rhex.clone();
                                }
                                ui.add_space(4.0);
                            }
                        });
                    });

                    ui.add_space(10.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_accent_btn(ui, pal, &cp_apply, 96.0, 28.0) {
                            let (ar, ag, ab) = hsv_to_rgb(h, s, v);
                            let final_hex = rgb_to_hex(ar, ag, ab);
                            let mut g = core.lock().unwrap();
                            g.recent_accents
                                .retain(|x| !x.eq_ignore_ascii_case(&final_hex));
                            g.recent_accents.insert(0, final_hex.clone());
                            g.recent_accents.truncate(5);
                            g.live.ui_settings.use_windows_accent = false;
                            g.live.ui_settings.custom_accent_hex = final_hex;
                            g.color_picker_open = false;
                            g.persist_ui_settings();
                            super::super::repaint_active_from_core(ui.ctx(), core);
                            return;
                        }
                        ui.add_space(6.0);
                        if small_secondary_btn(ui, pal, &cp_cancel, 78.0, 28.0) {
                            core.lock().unwrap().color_picker_open = false;
                        }
                    });
                });
        });

    let mut g = core.lock().unwrap();
    if g.color_picker_open {
        g.hsv_h = h;
        g.hsv_s = s;
        g.hsv_v = v;
        g.hsv_hex_buf = hex_buf;
    }
}
