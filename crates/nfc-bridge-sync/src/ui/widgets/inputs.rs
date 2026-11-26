use super::super::theme::ThemePalette;
use egui::{Align, Color32, CornerRadius, FontId, Layout, Margin, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

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
    let mono = FontId::monospace(12.0);
    let row_h = ui.fonts_mut(|f| f.row_height(&mono)).ceil().clamp(14.0, h - 4.0);
    let text_rect = Rect::from_center_size(
        box_r.center(),
        Vec2::new((box_r.width() - 20.0).max(20.0), row_h),
    );
    ui.painter().rect(
        box_r,
        CornerRadius::same(8),
        pal.surface,
        Stroke::new(1.0, pal.outline),
        StrokeKind::Inside,
    );
    let hint_col = if pal.is_dark {
        Color32::from_rgb(156, 163, 175)
    } else {
        pal.on_surface_muted
    };
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let te = ui.add_sized(
                Vec2::new(text_rect.width(), row_h),
                egui::TextEdit::singleline(buf)
                    .frame(egui::Frame::NONE)
                    .margin(Margin::ZERO)
                    .font(mono.clone())
                    .text_color(pal.on_surface)
                    .hint_text(RichText::new(hint).font(mono).color(hint_col))
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
