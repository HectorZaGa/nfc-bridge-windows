//! Sistema de diseño Material/Stitch y paleta dinámica Dark/Light + color de acento
//! para la interfaz nativa `egui` de NFC Bridge.

use crate::server::SyncUiSettings;
use egui::{Color32, CornerRadius, Stroke, Visuals};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub is_dark: bool,
    pub surface: Color32,
    pub surface_card: Color32,
    pub surface_elevated: Color32,
    pub on_surface: Color32,
    pub on_surface_elevated: Color32,
    pub on_surface_muted: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub accent_on_soft: Color32,
    pub accent_border: Color32,
    pub led_green: Color32,
    pub amber_pause: Color32,
    pub amber_soft: Color32,
    pub on_amber_soft: Color32,
    pub red_disconnect: Color32,
    pub red_soft: Color32,
    pub red_border: Color32,
}

impl ThemePalette {
    pub fn from_settings(settings: &SyncUiSettings, win_accent_hex: &str) -> Self {
        let is_dark = match settings.theme_mode.as_str() {
            "light" => false,
            "system" => detect_windows_dark_mode(),
            _ => true,
        };

        let active_hex = if settings.use_windows_accent {
            win_accent_hex
        } else {
            &settings.custom_accent_hex
        };
        let (r, g, b) = parse_hex_rgb(active_hex);
        let lum = (0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32)) / 255.0;

        let accent = Color32::from_rgb(r, g, b);
        let on_accent = if lum > 0.62 {
            Color32::from_rgb(17, 24, 39)
        } else {
            Color32::WHITE
        };

        let accent_soft = if is_dark {
            Color32::from_rgba_unmultiplied(r, g, b, 44)
        } else {
            Color32::from_rgba_unmultiplied(r, g, b, 30)
        };
        let accent_border = if is_dark {
            Color32::from_rgba_unmultiplied(r, g, b, 128)
        } else {
            Color32::from_rgba_unmultiplied(r, g, b, 108)
        };
        let accent_on_soft = if is_dark {
            Color32::from_rgb(
                ((r as f32) + (255.0 - r as f32) * 0.45).round().min(255.0) as u8,
                ((g as f32) + (255.0 - g as f32) * 0.45).round().min(255.0) as u8,
                ((b as f32) + (255.0 - b as f32) * 0.45).round().min(255.0) as u8,
            )
        } else {
            Color32::from_rgb(
                ((r as f32) * 0.72).round() as u8,
                ((g as f32) * 0.72).round() as u8,
                ((b as f32) * 0.72).round() as u8,
            )
        };

        if is_dark {
            Self {
                is_dark: true,
                surface: Color32::from_rgb(18, 18, 18),
                surface_card: Color32::from_rgb(30, 30, 30),
                surface_elevated: Color32::from_rgb(38, 38, 38),
                on_surface: Color32::from_rgb(243, 240, 239),
                on_surface_elevated: Color32::from_rgb(229, 226, 225),
                on_surface_muted: Color32::from_rgb(173, 171, 170),
                outline: Color32::from_rgb(49, 48, 48),
                outline_variant: Color32::from_rgb(71, 70, 70),
                accent,
                on_accent,
                accent_soft,
                accent_on_soft,
                accent_border,
                led_green: Color32::from_rgb(34, 197, 94),
                amber_pause: Color32::from_rgb(245, 158, 11),
                amber_soft: Color32::from_rgb(42, 31, 10),
                on_amber_soft: Color32::from_rgb(255, 185, 95),
                red_disconnect: Color32::from_rgb(255, 137, 130),
                red_soft: Color32::from_rgb(36, 20, 20),
                red_border: Color32::from_rgb(127, 41, 41),
            }
        } else {
            Self {
                is_dark: false,
                surface: Color32::from_rgb(245, 246, 248),
                surface_card: Color32::WHITE,
                surface_elevated: Color32::from_rgb(236, 238, 242),
                on_surface: Color32::from_rgb(20, 22, 26),
                on_surface_elevated: Color32::from_rgb(30, 34, 40),
                on_surface_muted: Color32::from_rgb(91, 98, 112),
                outline: Color32::from_rgb(217, 222, 231),
                outline_variant: Color32::from_rgb(194, 200, 212),
                accent,
                on_accent,
                accent_soft,
                accent_on_soft,
                accent_border,
                led_green: Color32::from_rgb(34, 197, 94),
                amber_pause: Color32::from_rgb(217, 119, 6),
                amber_soft: Color32::from_rgb(254, 243, 199),
                on_amber_soft: Color32::from_rgb(146, 64, 14),
                red_disconnect: Color32::from_rgb(220, 38, 38),
                red_soft: Color32::from_rgb(254, 226, 226),
                red_border: Color32::from_rgb(252, 165, 165),
            }
        }
    }
}

pub fn parse_hex_rgb(hex: &str) -> (u8, u8, u8) {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return (r, g, b);
        }
    }
    (59, 130, 246)
}

pub fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

pub fn hsv_to_rgb(h_deg: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h_deg.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let d = max - min;
    let s = if max == 0.0 { 0.0 } else { d / max };
    let h = if d == 0.0 {
        0.0
    } else if (max - rf).abs() < 1e-6 {
        60.0 * ((gf - bf) / d + if gf < bf { 6.0 } else { 0.0 })
    } else if (max - gf).abs() < 1e-6 {
        60.0 * ((bf - rf) / d + 2.0)
    } else {
        60.0 * ((rf - gf) / d + 4.0)
    };
    (h, s, max)
}

pub fn apply_egui_visuals(ctx: &egui::Context, pal: &ThemePalette) {
    let mut visuals = if pal.is_dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = pal.surface;
    visuals.window_fill = pal.surface_card;
    visuals.faint_bg_color = pal.surface_elevated;
    visuals.extreme_bg_color = pal.surface;
    visuals.override_text_color = Some(pal.on_surface);
    visuals.selection.bg_fill = pal.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0, pal.accent);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.window_stroke = Stroke::new(1.0, pal.outline);

    visuals.widgets.noninteractive.bg_fill = pal.surface_card;
    visuals.widgets.noninteractive.weak_bg_fill = pal.surface_elevated;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal.outline);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, pal.on_surface);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = pal.surface_elevated;
    visuals.widgets.inactive.weak_bg_fill = pal.surface_elevated;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, pal.outline);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, pal.on_surface);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = pal.surface_elevated;
    visuals.widgets.hovered.weak_bg_fill = pal.surface_elevated;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, pal.outline_variant);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, pal.on_surface);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);

    visuals.widgets.active.bg_fill = pal.accent_soft;
    visuals.widgets.active.weak_bg_fill = pal.accent_soft;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, pal.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, pal.on_surface);
    visuals.widgets.active.corner_radius = CornerRadius::same(8);

    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.scroll = egui::style::ScrollStyle {
            floating: true,
            bar_width: 5.0,
            handle_min_length: 20.0,
            bar_inner_margin: 3.0,
            bar_outer_margin: 2.0,
            floating_width: 4.0,
            floating_allocated_width: 0.0,
            dormant_background_opacity: 0.0,
            dormant_handle_opacity: 0.45,
            active_background_opacity: 0.15,
            active_handle_opacity: 0.9,
            interact_background_opacity: 0.25,
            interact_handle_opacity: 1.0,
            foreground_color: false,
            ..Default::default()
        };
    });
}

fn detect_windows_dark_mode() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("reg.exe")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "/v",
                "AppsUseLightTheme",
            ])
            .creation_flags(0x0800_0000)
            .output();
        if let Ok(o) = out {
            let txt = String::from_utf8_lossy(&o.stdout);
            if txt.contains("0x1") {
                return false;
            }
            if txt.contains("0x0") {
                return true;
            }
        }
    }
    true
}

/// Aplica esquinas redondeadas DWM de Windows 11, borde nativo y pincel de fondo oscuro/claro
/// al HWND del viewport activo para evitar bordes blancos en cualquier ventana frameless.
#[cfg(windows)]
pub fn apply_win32_dwm_to_title(title: &str, pal: &ThemePalette) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    use windows_sys::Win32::Graphics::Gdi::CreateSolidBrush;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetClassLongPtrW, GCLP_HBRBACKGROUND};

    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let corner_pref: u32 = DWMWCP_ROUND as u32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &corner_pref as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let b_r = pal.outline.r() as u32;
        let b_g = pal.outline.g() as u32;
        let b_b = pal.outline.b() as u32;
        let border_color: u32 = (b_b << 16) | (b_g << 8) | b_r;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &border_color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let dark_flag: i32 = if pal.is_dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark_flag as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );
        let bg_r = pal.surface.r() as u32;
        let bg_g = pal.surface.g() as u32;
        let bg_b = pal.surface.b() as u32;
        let bg_color: u32 = (bg_b << 16) | (bg_g << 8) | bg_r;
        let brush = CreateSolidBrush(bg_color);
        if !brush.is_null() {
            SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, brush as isize);
        }
    }
}
