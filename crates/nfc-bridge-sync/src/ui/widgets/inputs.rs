use super::super::theme::ThemePalette;
use egui::{Align, Color32, CornerRadius, FontId, Layout, Sense, Stroke, StrokeKind, Vec2};

pub(crate) fn styled_singleline_input(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    buf: &mut String,
    w: f32,
    h: f32,
    hint: &str,
) -> egui::Response {
    let (box_r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let mut inner_resp: Option<egui::Response> = None;
    let text_rect = box_r.shrink2(Vec2::new(10.0, 2.0));
    ui.painter().rect(
        box_r,
        CornerRadius::same(8),
        pal.surface,
        Stroke::new(1.0, pal.outline),
        StrokeKind::Inside,
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            let te = ui.add_sized(
                Vec2::new(text_rect.width(), 20.0),
                egui::TextEdit::singleline(buf)
                    .frame(egui::Frame::NONE)
                    .font(FontId::monospace(12.0))
                    .hint_text(hint)
                    .clip_text(true),
            );
            if te.has_focus() {
                ui.painter().rect_stroke(
                    box_r,
                    CornerRadius::same(8),
                    Stroke::new(1.0, pal.accent),
                    StrokeKind::Inside,
                );
            }
            inner_resp = Some(te);
        },
    );
    inner_resp.unwrap()
}

pub(crate) fn truncate_text_to_width(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    max_w: f32,
) -> String {
    if painter
        .layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE)
        .size()
        .x
        <= max_w
    {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cand: String = chars[..mid].iter().collect::<String>() + "...";
        if painter
            .layout_no_wrap(cand, font.clone(), Color32::WHITE)
            .size()
            .x
            <= max_w
        {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        "...".to_string()
    } else {
        chars[..lo].iter().collect::<String>() + "..."
    }
}
