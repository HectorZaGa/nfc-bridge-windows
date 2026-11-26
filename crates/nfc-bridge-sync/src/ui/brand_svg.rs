//! Rasterizador vectorial ligero para los SVG oficiales en `assets/icons/brands/`.
//! Genera máscaras blancas con canal alfa (recortes reales en ojos/espacios) para que
//! `egui::Painter::image` pueda teñirlas adaptativamente al color de acento o tema activo.

use egui::{Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use std::collections::HashMap;

const ANDROID_SVG: &str = include_str!("../../../../assets/icons/brands/Android_alt.svg");
const APPLE_SVG: &str = include_str!("../../../../assets/icons/brands/Apple.svg");
const WINDOWS_SVG: &str = include_str!("../../../../assets/icons/brands/Windows.svg");

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BrandIcon {
    Android,
    Apple,
    Windows,
}

impl BrandIcon {
    fn svg_source(self) -> &'static str {
        match self {
            BrandIcon::Android => ANDROID_SVG,
            BrandIcon::Apple => APPLE_SVG,
            BrandIcon::Windows => WINDOWS_SVG,
        }
    }

    fn tex_name(self) -> &'static str {
        match self {
            BrandIcon::Android => "nb_brand_android",
            BrandIcon::Apple => "nb_brand_apple",
            BrandIcon::Windows => "nb_brand_windows",
        }
    }
}

#[derive(Default)]
struct BrandTextureCache {
    textures: HashMap<BrandIcon, (TextureHandle, f32)>,
}

/// Pinta un ícono oficial de marca dentro de `rect` manteniendo su relación de aspecto
/// y teñido adaptativamente con `color`.
pub fn paint_brand_icon(painter: &egui::Painter, rect: Rect, icon: BrandIcon, color: Color32) {
    let ctx = painter.ctx().clone();
    let (tex, aspect) = {
        let arc = ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<std::sync::Arc<std::sync::Mutex<BrandTextureCache>>>(
                egui::Id::unique("nb_brand_tex_cache"),
            )
            .clone()
        });
        let mut cache = arc.lock().unwrap();
        if let Some((handle, asp)) = cache.textures.get(&icon) {
            (handle.clone(), *asp)
        } else {
            let (img, asp) = rasterize_svg_mask(icon.svg_source(), 128);
            let handle = ctx.load_texture(icon.tex_name(), img, TextureOptions::LINEAR);
            cache.textures.insert(icon, (handle.clone(), asp));
            (handle, asp)
        }
    };

    let fit = fit_aspect_rect(rect, aspect);
    painter.image(
        tex.id(),
        fit,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        color,
    );
}

fn fit_aspect_rect(outer: Rect, aspect_w_over_h: f32) -> Rect {
    let ow = outer.width().max(1.0);
    let oh = outer.height().max(1.0);
    let aspect = aspect_w_over_h.max(0.05);
    let (w, h) = if ow / oh > aspect {
        (oh * aspect, oh)
    } else {
        (ow, ow / aspect)
    };
    Rect::from_center_size(outer.center(), Vec2::new(w, h))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FillRule {
    NonZero,
    EvenOdd,
}

type Poly = Vec<(f32, f32)>;

struct PathGroup {
    polys: Vec<Poly>,
    rule: FillRule,
}

pub(crate) fn rasterize_svg_mask(svg: &str, max_dim: usize) -> (ColorImage, f32) {
    let (vx, vy, vw, vh) = parse_viewbox(svg).unwrap_or((0.0, 0.0, 24.0, 24.0));
    let paths = extract_paths(svg);

    let aspect = (vw / vh.max(1e-3)).max(0.05);
    let (out_w, out_h) = if aspect >= 1.0 {
        let h = ((max_dim as f32) / aspect).round().max(8.0) as usize;
        (max_dim, h)
    } else {
        let w = ((max_dim as f32) * aspect).round().max(8.0) as usize;
        (w, max_dim)
    };

    let scale_x = out_w as f32 / vw.max(1e-3);
    let scale_y = out_h as f32 / vh.max(1e-3);

    let mut flattened_paths: Vec<PathGroup> = Vec::new();
    for (d, rule) in paths {
        let polys = parse_and_flatten_path(&d, vx, vy, scale_x, scale_y);
        if !polys.is_empty() {
            flattened_paths.push(PathGroup { polys, rule });
        }
    }

    let mut rgba = vec![0u8; out_w * out_h * 4];
    const SUB: usize = 4;
    let inv_samples = 1.0 / (SUB * SUB) as f32;

    for py in 0..out_h {
        for px in 0..out_w {
            let mut hits = 0usize;
            for sy in 0..SUB {
                let y = py as f32 + (sy as f32 + 0.5) / SUB as f32;
                for sx in 0..SUB {
                    let x = px as f32 + (sx as f32 + 0.5) / SUB as f32;
                    let inside = flattened_paths
                        .iter()
                        .any(|pg| point_in_polygons(x, y, &pg.polys, pg.rule));
                    if inside {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let idx = (py * out_w + px) * 4;
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = ((hits as f32 * inv_samples) * 255.0).round() as u8;
            }
        }
    }

    (ColorImage::from_rgba_unmultiplied([out_w, out_h], &rgba), aspect)
}

fn parse_viewbox(svg: &str) -> Option<(f32, f32, f32, f32)> {
    let key = "viewBox=\"";
    let start = svg.find(key)? + key.len();
    let end = svg[start..].find('"')? + start;
    let nums: Vec<f32> = svg[start..end]
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();
    if nums.len() == 4 {
        Some((nums[0], nums[1], nums[2], nums[3]))
    } else {
        None
    }
}

fn extract_attr(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(pos) = tag.find(&needle) {
            let rest = &tag[pos + needle.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

fn extract_paths(svg: &str) -> Vec<(String, FillRule)> {
    let svg_default_rule = if svg.contains("fill-rule:evenodd") && !svg.contains("fill-rule:nonzero") {
        FillRule::EvenOdd
    } else {
        FillRule::NonZero
    };
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(pos) = rest.find("<path") {
        let after = &rest[pos..];
        let Some(end) = after.find('>') else { break };
        let tag = &after[..=end];
        if let Some(d) = extract_attr(tag, "d") {
            let rule = if tag.contains("fill-rule:nonzero") || tag.contains("fill-rule=\"nonzero\"") {
                FillRule::NonZero
            } else if tag.contains("fill-rule:evenodd") || tag.contains("fill-rule=\"evenodd\"") {
                FillRule::EvenOdd
            } else {
                svg_default_rule
            };
            out.push((d, rule));
        }
        rest = &after[end + 1..];
    }
    out
}

enum Token {
    Cmd(char),
    Num(f32),
}

fn tokenize_path(d: &str) -> Vec<Token> {
    let bytes = d.as_bytes();
    let mut i = 0;
    let mut tokens = Vec::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b == b',' {
            i += 1;
            continue;
        }
        if b.is_ascii_alphabetic() {
            tokens.push(Token::Cmd(b as char));
            i += 1;
            continue;
        }
        let start = i;
        if b == b'-' || b == b'+' {
            i += 1;
        }
        let mut seen_dot = false;
        while i < bytes.len() {
            let c = bytes[i];
            if c.is_ascii_digit() {
                i += 1;
            } else if c == b'.' && !seen_dot {
                seen_dot = true;
                i += 1;
            } else if (c == b'e' || c == b'E') && i > start {
                i += 1;
                if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
                    i += 1;
                }
            } else {
                break;
            }
        }
        if i > start {
            if let Ok(v) = d[start..i].parse::<f32>() {
                tokens.push(Token::Num(v));
            }
        } else {
            i += 1;
        }
    }
    tokens
}

fn parse_and_flatten_path(
    d: &str,
    vx: f32,
    vy: f32,
    sx: f32,
    sy: f32,
) -> Vec<Vec<(f32, f32)>> {
    let tokens = tokenize_path(d);
    let mut polys: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut current: Vec<(f32, f32)> = Vec::new();
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    let mut start_x = 0.0f32;
    let mut start_y = 0.0f32;
    let mut last_cubic_ctrl: Option<(f32, f32)> = None;
    let mut last_quad_ctrl: Option<(f32, f32)> = None;
    let mut idx = 0usize;
    let mut active_cmd = ' ';

    let to_px = |x: f32, y: f32| -> (f32, f32) { ((x - vx) * sx, (y - vy) * sy) };

    let next_num = |idx: &mut usize| -> Option<f32> {
        if let Some(Token::Num(v)) = tokens.get(*idx) {
            *idx += 1;
            Some(*v)
        } else {
            None
        }
    };

    while idx < tokens.len() {
        if let Token::Cmd(c) = tokens[idx] {
            active_cmd = c;
            idx += 1;
            if c == 'Z' || c == 'z' {
                if current.len() >= 3 {
                    polys.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                cx = start_x;
                cy = start_y;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                continue;
            }
        }

        let is_rel = active_cmd.is_ascii_lowercase();
        let cmd_u = active_cmd.to_ascii_uppercase();
        match cmd_u {
            'M' => {
                let (Some(x), Some(y)) = (next_num(&mut idx), next_num(&mut idx)) else {
                    break;
                };
                if current.len() >= 3 {
                    polys.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                cx = if is_rel { cx + x } else { x };
                cy = if is_rel { cy + y } else { y };
                start_x = cx;
                start_y = cy;
                current.push(to_px(cx, cy));
                active_cmd = if is_rel { 'l' } else { 'L' };
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'L' => {
                let (Some(x), Some(y)) = (next_num(&mut idx), next_num(&mut idx)) else {
                    break;
                };
                cx = if is_rel { cx + x } else { x };
                cy = if is_rel { cy + y } else { y };
                current.push(to_px(cx, cy));
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'H' => {
                let Some(x) = next_num(&mut idx) else { break };
                cx = if is_rel { cx + x } else { x };
                current.push(to_px(cx, cy));
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'V' => {
                let Some(y) = next_num(&mut idx) else { break };
                cy = if is_rel { cy + y } else { y };
                current.push(to_px(cx, cy));
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'C' => {
                let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                ) else {
                    break;
                };
                let (ax1, ay1) = if is_rel { (cx + x1, cy + y1) } else { (x1, y1) };
                let (ax2, ay2) = if is_rel { (cx + x2, cy + y2) } else { (x2, y2) };
                let (ax, ay) = if is_rel { (cx + x, cy + y) } else { (x, y) };
                push_cubic(&mut current, to_px(cx, cy), to_px(ax1, ay1), to_px(ax2, ay2), to_px(ax, ay));
                cx = ax;
                cy = ay;
                last_cubic_ctrl = Some((ax2, ay2));
                last_quad_ctrl = None;
            }
            'S' => {
                let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                ) else {
                    break;
                };
                let (ax1, ay1) = match last_cubic_ctrl {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                let (ax2, ay2) = if is_rel { (cx + x2, cy + y2) } else { (x2, y2) };
                let (ax, ay) = if is_rel { (cx + x, cy + y) } else { (x, y) };
                push_cubic(&mut current, to_px(cx, cy), to_px(ax1, ay1), to_px(ax2, ay2), to_px(ax, ay));
                cx = ax;
                cy = ay;
                last_cubic_ctrl = Some((ax2, ay2));
                last_quad_ctrl = None;
            }
            'Q' => {
                let (Some(x1), Some(y1), Some(x), Some(y)) = (
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                ) else {
                    break;
                };
                let (ax1, ay1) = if is_rel { (cx + x1, cy + y1) } else { (x1, y1) };
                let (ax, ay) = if is_rel { (cx + x, cy + y) } else { (x, y) };
                push_quad(&mut current, to_px(cx, cy), to_px(ax1, ay1), to_px(ax, ay));
                cx = ax;
                cy = ay;
                last_quad_ctrl = Some((ax1, ay1));
                last_cubic_ctrl = None;
            }
            'T' => {
                let (Some(x), Some(y)) = (next_num(&mut idx), next_num(&mut idx)) else {
                    break;
                };
                let (ax1, ay1) = match last_quad_ctrl {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                let (ax, ay) = if is_rel { (cx + x, cy + y) } else { (x, y) };
                push_quad(&mut current, to_px(cx, cy), to_px(ax1, ay1), to_px(ax, ay));
                cx = ax;
                cy = ay;
                last_quad_ctrl = Some((ax1, ay1));
                last_cubic_ctrl = None;
            }
            _ => {
                idx += 1;
            }
        }
    }

    if current.len() >= 3 {
        polys.push(current);
    }
    polys
}

fn push_cubic(out: &mut Vec<(f32, f32)>, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) {
    let dist = ((p1.0 - p0.0).hypot(p1.1 - p0.1)
        + (p2.0 - p1.0).hypot(p2.1 - p1.1)
        + (p3.0 - p2.0).hypot(p3.1 - p2.1))
    .max(1.0);
    let steps = (dist * 1.2).ceil().clamp(6.0, 48.0) as usize;
    for s in 1..=steps {
        let t = s as f32 / steps as f32;
        let u = 1.0 - t;
        let x = u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0;
        let y = u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1;
        out.push((x, y));
    }
}

fn push_quad(out: &mut Vec<(f32, f32)>, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32)) {
    let dist = ((p1.0 - p0.0).hypot(p1.1 - p0.1) + (p2.0 - p1.0).hypot(p2.1 - p1.1)).max(1.0);
    let steps = (dist * 1.2).ceil().clamp(4.0, 32.0) as usize;
    for s in 1..=steps {
        let t = s as f32 / steps as f32;
        let u = 1.0 - t;
        let x = u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0;
        let y = u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1;
        out.push((x, y));
    }
}

fn point_in_polygons(x: f32, y: f32, polys: &[Vec<(f32, f32)>], rule: FillRule) -> bool {
    match rule {
        FillRule::EvenOdd => {
            let mut inside = false;
            for poly in polys {
                let n = poly.len();
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = poly[i];
                    let (xj, yj) = poly[j];
                    if (yi > y) != (yj > y) {
                        let ix = (xj - xi) * (y - yi) / (yj - yi) + xi;
                        if x < ix {
                            inside = !inside;
                        }
                    }
                    j = i;
                }
            }
            inside
        }
        FillRule::NonZero => {
            let mut winding = 0i32;
            for poly in polys {
                let n = poly.len();
                let mut j = n - 1;
                for i in 0..n {
                    let (x0, y0) = poly[j];
                    let (x1, y1) = poly[i];
                    if y0 <= y {
                        if y1 > y && ((x1 - x0) * (y - y0) - (x - x0) * (y1 - y0)) > 0.0 {
                            winding += 1;
                        }
                    } else if y1 <= y && ((x1 - x0) * (y - y0) - (x - x0) * (y1 - y0)) < 0.0 {
                        winding -= 1;
                    }
                    j = i;
                }
            }
            winding != 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(img: &ColorImage, fx: f32, fy: f32) -> u8 {
        let [w, h] = img.size;
        let x = ((w as f32) * fx).round().clamp(0.0, (w - 1) as f32) as usize;
        let y = ((h as f32) * fy).round().clamp(0.0, (h - 1) as f32) as usize;
        img.pixels[y * w + x].a()
    }

    #[test]
    fn test_official_brand_svgs_rasterize_with_cutouts() {
        let (android_img, a_asp) = rasterize_svg_mask(ANDROID_SVG, 128);
        assert!(a_asp > 1.5);
        // Frente del androide opaca
        assert_eq!(alpha_at(&android_img, 0.50, 0.50), 255);
        // Ojos recortados (transparentes)
        assert_eq!(alpha_at(&android_img, 0.27, 0.67), 0);
        assert_eq!(alpha_at(&android_img, 0.73, 0.67), 0);
        // Esquina superior izquierda transparente
        assert_eq!(alpha_at(&android_img, 0.02, 0.05), 0);

        let (apple_img, ap_asp) = rasterize_svg_mask(APPLE_SVG, 128);
        assert!(ap_asp < 1.0);
        assert_eq!(alpha_at(&apple_img, 0.45, 0.60), 255);
        assert_eq!(alpha_at(&apple_img, 0.02, 0.02), 0);

        let (win_img, w_asp) = rasterize_svg_mask(WINDOWS_SVG, 128);
        assert!((w_asp - 1.0).abs() < 0.05);
        assert_eq!(alpha_at(&win_img, 0.20, 0.20), 255);
        assert_eq!(alpha_at(&win_img, 0.50, 0.50), 0);
    }
}
