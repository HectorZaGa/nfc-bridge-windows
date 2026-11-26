//! Interfaz de escritorio 100 % Rust nativa (`egui` + `eframe`) para NFC Bridge Sync y Tools Desktop.
//!
//! Ventanas independientes (Viewports nativos sin HTML ni WebView2):
//! - Ventana principal sin bordes (`860x566` adaptativa) que se minimiza a la bandeja al cerrarse.
//! - Ventana de Opciones independiente (`680x468`) con sidebar colapsable sin salto y selector HSV.
//! - Menú emergente personalizado en System Tray (`296px`) fiel al diseño Stitch.

#![cfg(target_os = "windows")]

use crate::egui_i18n::I18nCatalog;
use crate::egui_icons::{
    glyph, paint_chevron_y, paint_device_os_icon, paint_glyph, paint_refresh_spin,
    paint_update_down_badge, semibold, setup_fonts,
};
use crate::egui_qr::paint_qr_box;
use crate::egui_theme::{
    apply_egui_visuals, apply_win32_dwm_to_title, hsv_to_rgb, parse_hex_rgb, rgb_to_hex,
    rgb_to_hsv, ThemePalette,
};
use crate::server::{DesktopLiveState, DriverIpcClient, IpcCommand};
use crate::updater::{DesktopUpdater, UpdatesOverviewState};
use eframe::egui;
use egui::{
    Align, Color32, CornerRadius, FontId, Id, Layout, Margin, Mesh, Pos2, Rect, RichText, Sense,
    Stroke, StrokeKind, Vec2, ViewportBuilder, ViewportCommand, ViewportId,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Clone)]
struct IssuedQrView {
    method: String,
    pair_token: String,
    expires_at: Instant,
    uri: String,
    share_link: Option<String>,
}

struct SharedUiCore {
    ipc: DriverIpcClient,
    updater: DesktopUpdater,
    rt: tokio::runtime::Handle,
    i18n: I18nCatalog,
    app_title: String,

    live: DesktopLiveState,
    updates: UpdatesOverviewState,

    // Control de ventanas
    main_visible: bool,
    main_want_focus: bool,
    options_open: bool,
    options_want_focus: bool,
    tray_open: bool,
    tray_opened_at: Option<Instant>,
    tray_pos: Option<Pos2>,
    tray_devices_expanded: bool,
    should_quit: bool,

    // Estado visual de la ventana principal
    active_panel: String,
    qr_method: Option<String>,
    qr_data: Option<IssuedQrView>,
    qr_error: Option<String>,
    qr_loading: bool,
    live_relay_link: Option<String>,
    live_relay_expires_at: Option<Instant>,

    // Perfil / dropdown / renombrado inline
    device_dropdown_open: bool,
    editing_device_id: Option<String>,
    editing_device_name: String,

    // Inputs y feedback con protección anti-sobrescritura mientras se editan/guardan
    relay_url_buf: String,
    relay_url_inited: bool,

    options_tab: OptionsTab,
    sidebar_compact: bool,
    language_dropdown_open: bool,

    net_ip_buf: String,
    net_port_buf: String,
    net_dirty_until: Option<Instant>,
    net_feedback: Option<(String, bool, Instant)>,

    ui_settings_dirty_until: Option<Instant>,
    auto_mobile_dirty_until: Option<Instant>,
    auto_mobile_optimistic: bool,

    // Selector HSV flotante
    color_picker_open: bool,
    color_picker_anchor: Pos2,
    hsv_h: f32,
    hsv_s: f32,
    hsv_v: f32,
    hsv_hex_buf: String,
    recent_accents: Vec<String>,

    // Misc
    copied_key: Option<(String, Instant)>,
    upd_spin_until: Option<Instant>,
    restart_dismissed: bool,
    last_dwm_main_key: Option<(bool, Color32)>,
    last_dwm_opts_key: Option<(bool, Color32)>,
    last_dwm_tray_key: Option<(bool, Color32)>,
    last_visuals_pal: Option<ThemePalette>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OptionsTab {
    Appearance,
    Network,
    Updates,
    About,
}

impl SharedUiCore {
    fn current_lang(&self) -> &str {
        &self.live.ui_settings.language
    }

    fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.i18n.t(self.current_lang(), key)
    }

    fn t_vars(&self, key: &str, vars: &[(&str, &str)]) -> String {
        self.i18n.t_vars(self.current_lang(), key, vars)
    }

    fn palette(&self) -> ThemePalette {
        ThemePalette::from_settings(&self.live.ui_settings, &self.live.windows_accent_hex)
    }

    fn persist_ui_settings(&mut self) {
        self.ui_settings_dirty_until = Some(Instant::now() + Duration::from_millis(1800));
        let settings = self.live.ui_settings.clone();
        let ipc = self.ipc.clone();
        self.rt.spawn(async move {
            ipc.update_ui_settings(settings).await;
        });
    }

    fn send_ipc(&self, cmd: IpcCommand) {
        let ipc = self.ipc.clone();
        self.rt.spawn(async move {
            let _ = ipc.send_command(cmd).await;
            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
        });
    }
}

pub fn run_desktop_ui(
    app_title: &str,
    ipc: DriverIpcClient,
    updater: DesktopUpdater,
    rt_handle: tokio::runtime::Handle,
    _data_dir: std::path::PathBuf,
) -> anyhow::Result<()> {
    let rgba_32 = build_nfc_rgba_bytes();
    let eframe_icon = egui::IconData {
        rgba: rgba_32.clone(),
        width: 32,
        height: 32,
    };
    let tray_icon_img = tray_icon::Icon::from_rgba(rgba_32, 32, 32).ok();

    let tray_handle = tray_icon_img.and_then(|ic| {
        TrayIconBuilder::new()
            .with_tooltip(app_title)
            .with_icon(ic)
            .build()
            .ok()
    });

    let init_live = rt_handle.block_on(async { ipc.get_live_state().await });
    let init_updates = rt_handle.block_on(async { updater.snapshot().await });
    let init_pal =
        ThemePalette::from_settings(&init_live.ui_settings, &init_live.windows_accent_hex);

    let (r0, g0, b0) = parse_hex_rgb(&init_live.ui_settings.custom_accent_hex);
    let (h0, s0, v0) = rgb_to_hsv(r0, g0, b0);

    let core = Arc::new(Mutex::new(SharedUiCore {
        ipc: ipc.clone(),
        updater: updater.clone(),
        rt: rt_handle.clone(),
        i18n: I18nCatalog::load(),
        app_title: app_title.to_string(),
        sidebar_compact: init_live.ui_settings.settings_sidebar_compact,
        auto_mobile_optimistic: init_live.snapshot.auto_accept_mobile_switch,
        net_ip_buf: init_live.snapshot.tcp_ip_override.clone().unwrap_or_default(),
        net_port_buf: init_live.snapshot.tcp_port.to_string(),
        relay_url_buf: init_live.snapshot.relay_url.clone().unwrap_or_default(),
        relay_url_inited: init_live
            .snapshot
            .relay_url
            .as_deref()
            .is_some_and(|u| !u.is_empty()),
        live: init_live,
        updates: init_updates,
        main_visible: true,
        main_want_focus: false,
        options_open: false,
        options_want_focus: false,
        tray_open: false,
        tray_opened_at: None,
        tray_pos: None,
        tray_devices_expanded: false,
        should_quit: false,
        active_panel: "usb".to_string(),
        qr_method: None,
        qr_data: None,
        qr_error: None,
        qr_loading: false,
        live_relay_link: None,
        live_relay_expires_at: None,
        device_dropdown_open: false,
        editing_device_id: None,
        editing_device_name: String::new(),
        options_tab: OptionsTab::Appearance,
        language_dropdown_open: false,
        net_dirty_until: None,
        net_feedback: None,
        ui_settings_dirty_until: None,
        auto_mobile_dirty_until: None,
        color_picker_open: false,
        color_picker_anchor: Pos2::new(420.0, 220.0),
        hsv_h: if h0 == 0.0 && s0 == 0.0 { 217.0 } else { h0 },
        hsv_s: s0.max(0.05),
        hsv_v: v0.max(0.15),
        hsv_hex_buf: rgb_to_hex(r0, g0, b0),
        recent_accents: vec![
            "#06B6D4".to_string(),
            "#10B981".to_string(),
            "#8B5CF6".to_string(),
            "#EC4899".to_string(),
            "#F59E0B".to_string(),
        ],
        copied_key: None,
        upd_spin_until: None,
        restart_dismissed: false,
        last_dwm_main_key: None,
        last_dwm_opts_key: None,
        last_dwm_tray_key: None,
        last_visuals_pal: None,
    }));

    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title(app_title)
            .with_inner_size([860.0, 504.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(false)
            .with_icon(Arc::new(eframe_icon)),
        ..Default::default()
    };

    let tray_events: Arc<Mutex<Vec<TrayIconEvent>>> = Arc::new(Mutex::new(Vec::new()));

    let title_owned = app_title.to_string();
    eframe::run_native(
        &title_owned,
        native_options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            apply_egui_visuals(&cc.egui_ctx, &init_pal);
            {
                core.lock().unwrap().last_visuals_pal = Some(init_pal);
            }

            let ctx_tray = cc.egui_ctx.clone();
            let q_tray = tray_events.clone();
            TrayIconEvent::set_event_handler(Some(move |ev| {
                if let Ok(mut q) = q_tray.lock() {
                    q.push(ev);
                }
                ctx_tray.request_repaint();
            }));

            let ctx_bg = cc.egui_ctx.clone();
            let core_bg = core.clone();
            let ipc_bg = ipc.clone();
            let upd_bg = updater.clone();
            rt_handle.spawn(async move {
                let mut last_sig = String::new();
                loop {
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    let _ = ipc_bg.send_command(IpcCommand::GetTransportSnapshot).await;
                    let st = ipc_bg.get_live_state().await;
                    upd_bg
                        .set_known_driver_version(st.snapshot.driver_version.clone())
                        .await;
                    let upd = upd_bg.snapshot().await;
                    let sig = serde_json::to_string(&(&st, &upd)).unwrap_or_default();
                    let changed = sig != last_sig;
                    if changed {
                        last_sig = sig;
                    }
                    {
                        let mut guard = core_bg.lock().unwrap();
                        let now = Instant::now();
                        let keep_ui_settings = guard
                            .ui_settings_dirty_until
                            .is_some_and(|t| now < t);
                        let saved_ui = if keep_ui_settings {
                            Some(guard.live.ui_settings.clone())
                        } else {
                            None
                        };
                        guard.live = st;
                        if let Some(prev) = saved_ui {
                            guard.live.ui_settings = prev;
                        } else {
                            guard.sidebar_compact = guard.live.ui_settings.settings_sidebar_compact;
                        }
                        guard.updates = upd;

                        if !guard.relay_url_inited {
                            if let Some(ref ru) = guard.live.snapshot.relay_url {
                                if !ru.is_empty() {
                                    guard.relay_url_buf = ru.clone();
                                    guard.relay_url_inited = true;
                                }
                            }
                        }
                    }
                    if changed {
                        ctx_bg.request_repaint();
                    }
                }
            });

            Ok(Box::new(NfcBridgeEguiApp {
                core,
                _tray: tray_handle,
                tray_events,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}

struct NfcBridgeEguiApp {
    core: Arc<Mutex<SharedUiCore>>,
    _tray: Option<TrayIcon>,
    tray_events: Arc<Mutex<Vec<TrayIconEvent>>>,
}

impl eframe::App for NfcBridgeEguiApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let g = self.core.lock().unwrap();
        g.last_visuals_pal
            .unwrap_or_else(|| g.palette())
            .surface
            .to_normalized_gamma_f32()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Procesar eventos del icono del System Tray (despertado instantáneo por event handler)
        let pending_tray: Vec<TrayIconEvent> = {
            if let Ok(mut q) = self.tray_events.lock() {
                std::mem::take(&mut *q)
            } else {
                Vec::new()
            }
        };
        for evt in pending_tray {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = evt
            {
                let mut g = self.core.lock().unwrap();
                match button {
                    MouseButton::Left => {
                        g.tray_open = false;
                        g.main_visible = true;
                        g.main_want_focus = true;
                    }
                    MouseButton::Right => {
                        if g.tray_open {
                            g.tray_open = false;
                        } else {
                            let ppp = ctx.pixels_per_point().max(1.0);
                            g.tray_pos = Some(Pos2::new(
                                (position.x as f32) / ppp,
                                (position.y as f32) / ppp,
                            ));
                            g.tray_open = true;
                            g.tray_opened_at = Some(Instant::now());
                        }
                    }
                    _ => {}
                }
                ctx.request_repaint();
            }
        }

        // 2. Verificar cierre de ventana principal -> ocultar al System Tray (no salir)
        if ctx.input(|i| i.viewport().close_requested()) {
            let mut g = self.core.lock().unwrap();
            if !g.should_quit {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                g.main_visible = false;
            }
        }

        // 3. Sincronizar estado de foco/visibilidad y tema de ventana principal (solo si cambió)
        let (
            want_focus_main,
            main_title,
            pal,
            options_open,
            options_want_focus,
            tray_open,
            tray_pos,
            tray_h,
            should_quit,
            needs_dwm_main,
            needs_visuals,
        ) = {
            let mut g = self.core.lock().unwrap();
            let pal = g.palette();
            let need_vis = g.last_visuals_pal != Some(pal);
            if need_vis {
                g.last_visuals_pal = Some(pal);
            }

            let dwm_key = (pal.is_dark, pal.outline);
            let need_dwm = g.last_dwm_main_key != Some(dwm_key);
            if need_dwm {
                g.last_dwm_main_key = Some(dwm_key);
            }

            let wf = g.main_want_focus;
            g.main_want_focus = false;
            let owf = g.options_want_focus;
            g.options_want_focus = false;

            let th = estimate_tray_height(&g);
            (
                wf,
                g.app_title.clone(),
                pal,
                g.options_open,
                owf,
                g.tray_open,
                g.tray_pos,
                th,
                g.should_quit,
                need_dwm,
                need_vis,
            )
        };

        if needs_visuals {
            apply_egui_visuals(ctx, &pal);
        }

        if should_quit {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        if want_focus_main {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }

        if needs_dwm_main {
            apply_win32_dwm_to_title(&main_title, &pal);
        }

        if !options_open || !tray_open {
            let mut g = self.core.lock().unwrap();
            if !options_open {
                g.last_dwm_opts_key = None;
            }
            if !tray_open {
                g.last_dwm_tray_key = None;
            }
        }

        // 4. Ventana independiente de Opciones (Deferred Viewport)
        if options_open {
            let opt_id = ViewportId::from_hash_of("nb_options_viewport");
            let opt_title = format!("{} - Options", main_title);
            let mut vb = ViewportBuilder::default()
                .with_title(opt_title.clone())
                .with_inner_size([680.0, 468.0])
                .with_decorations(false)
                .with_resizable(false)
                .with_transparent(false);
            if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                let x = outer.center().x - 340.0;
                let y = outer.center().y - 234.0;
                vb = vb.with_position(Pos2::new(x.max(20.0), y.max(20.0)));
            }
            if options_want_focus {
                ctx.send_viewport_cmd_to(opt_id, ViewportCommand::Focus);
            }
            let core_opt = self.core.clone();
            ctx.show_viewport_deferred(opt_id, vb, move |ui, _class| {
                render_options_viewport(&core_opt, &opt_title, ui);
            });
        }

        // 5. Popup independiente del System Tray (Deferred Viewport)
        if tray_open {
            let tray_id = ViewportId::from_hash_of("nb_tray_popup_viewport");
            let tray_title = format!("{} - Tray", main_title);
            let w = 296.0_f32;
            let pos = if let Some(p) = tray_pos {
                Pos2::new((p.x - w + 18.0).max(12.0), (p.y - tray_h - 14.0).max(12.0))
            } else {
                Pos2::new(900.0, 420.0)
            };
            let vb = ViewportBuilder::default()
                .with_title(tray_title.clone())
                .with_inner_size([w, tray_h])
                .with_position(pos)
                .with_decorations(false)
                .with_resizable(false)
                .with_transparent(false)
                .with_always_on_top()
                .with_taskbar(false);
            let core_tray = self.core.clone();
            ctx.show_viewport_deferred(tray_id, vb, move |ui, _class| {
                render_tray_viewport(&core_tray, &tray_title, ui);
            });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        render_main_viewport(&self.core, &ctx, ui);
    }
}

fn estimate_tray_height(g: &SharedUiCore) -> f32 {
    let base = 368.0_f32;
    if g.tray_devices_expanded {
        let count = g.live.snapshot.trusted_devices.len().max(1).min(4) as f32;
        base + count * 30.0 + 6.0
    } else {
        base
    }
}

// ==================== 1. MAIN VIEWPORT ====================

fn render_main_viewport(core: &Arc<Mutex<SharedUiCore>>, ctx: &egui::Context, ui: &mut egui::Ui) {
    let pal = { core.lock().unwrap().palette() };

    // Auto-renovar enlace live de Relay cada ~85 s si aplica
    {
        let mut g = core.lock().unwrap();
        let need_relay_refresh = g.live.driver_online
            && g.live
                .snapshot
                .relay_url
                .as_deref()
                .is_some_and(|u| !u.is_empty())
            && g.live_relay_expires_at
                .is_none_or(|exp| Instant::now() >= exp);
        if need_relay_refresh {
            g.live_relay_expires_at = Some(Instant::now() + Duration::from_secs(75));
            let ipc = g.ipc.clone();
            let core_c = core.clone();
            let ctx_c = ctx.clone();
            g.rt.spawn(async move {
                if let Ok(qr) = ipc.request_pairing_qr("relay").await {
                    let mut guard = core_c.lock().unwrap();
                    guard.live_relay_link = qr.share_link;
                    guard.live_relay_expires_at = Some(Instant::now() + Duration::from_secs(75));
                }
                ctx_c.request_repaint();
            });
        }
    }

    egui::Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.outline))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // 1) Custom Frameless Titlebar (48 px)
            let (titlebar_rect, titlebar_resp) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::click_and_drag());
            ui.painter().rect_filled(titlebar_rect, CornerRadius::ZERO, pal.surface_card);
            ui.painter().line_segment(
                [titlebar_rect.left_bottom(), titlebar_rect.right_bottom()],
                Stroke::new(1.0, pal.outline),
            );

            let (
                app_title_text,
                pc_name,
                ver_text,
                has_update,
                is_tools,
                active_panel,
                qr_method,
            ) = {
                let g = core.lock().unwrap();
                let is_t = g.updates.app_product == "tools";
                let title = if !g.updates.app_display_name.is_empty() {
                    g.updates.app_display_name.clone()
                } else {
                    g.t(if is_t { "app_tools_name" } else { "app_sync_name" }).to_string()
                };
                let pc = if g.live.snapshot.pc_name.is_empty() {
                    g.t("pc_windows_default").to_string()
                } else {
                    g.live.snapshot.pc_name.clone()
                };
                let ver = format!("v{} · {}", g.live.sync_version, g.live.sync_build_id);
                let has_upd = g.updates.app_status == "available"
                    || g.updates.app_status == "installed_restart"
                    || g.updates.driver_status == "available";
                (
                    title,
                    pc,
                    ver,
                    has_upd,
                    is_t,
                    g.active_panel.clone(),
                    g.qr_method.clone(),
                )
            };
            let _ = is_tools;

            // Contenido izquierda del titlebar
            let logo_rect = Rect::from_min_size(
                Pos2::new(titlebar_rect.left() + 16.0, titlebar_rect.center().y - 14.0),
                Vec2::splat(28.0),
            );
            ui.painter().rect(
                logo_rect,
                CornerRadius::same(8),
                pal.accent_soft,
                Stroke::new(1.0, pal.accent_border),
                StrokeKind::Inside,
            );
            paint_glyph(ui.painter(), logo_rect.center(), glyph::CONTACTLESS, 16.0, pal.accent);

            let mut x_cur = logo_rect.right() + 8.0;
            let title_galley = ui.painter().layout_no_wrap(
                app_title_text,
                semibold(16.0),
                pal.on_surface,
            );
            let title_w = title_galley.size().x;
            ui.painter().galley(
                Pos2::new(x_cur, titlebar_rect.center().y - title_galley.size().y * 0.5),
                title_galley,
                pal.on_surface,
            );
            x_cur += title_w + 8.0;

            x_cur = paint_pill(ui, Pos2::new(x_cur, titlebar_rect.center().y), &pc_name, 12.0, false, false, &pal);
            x_cur += 8.0;
            let _ = paint_pill(ui, Pos2::new(x_cur, titlebar_rect.center().y), &ver_text, 10.0, true, true, &pal);

            // Botones derecha del titlebar: Settings, Minimize, Close (to tray) — 32x32 px con gap 4px
            let right = titlebar_rect.right() - 16.0;
            let cy = titlebar_rect.center().y;
            let close_rect = Rect::from_center_size(Pos2::new(right - 16.0, cy), Vec2::splat(32.0));
            let min_rect = Rect::from_center_size(Pos2::new(close_rect.left() - 20.0, cy), Vec2::splat(32.0));
            let opt_rect = Rect::from_center_size(Pos2::new(min_rect.left() - 20.0, cy), Vec2::splat(32.0));

            let opt_resp = ui.interact(opt_rect, Id::unique("tb_opt"), Sense::click());
            let min_resp = ui.interact(min_rect, Id::unique("tb_min"), Sense::click());
            let close_resp = ui.interact(close_rect, Id::unique("tb_close"), Sense::click());

            let hovering_chrome = opt_resp.hovered() || min_resp.hovered() || close_resp.hovered();
            if titlebar_resp.drag_started() && !hovering_chrome {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }

            paint_chrome_btn(ui.painter(), opt_rect, &opt_resp, glyph::SETTINGS, false, &pal);
            if has_update {
                paint_update_down_badge(ui.painter(), opt_rect, pal.surface_card, pal.accent);
            }
            paint_chrome_btn(ui.painter(), min_rect, &min_resp, glyph::REMOVE, false, &pal);
            paint_chrome_btn(ui.painter(), close_rect, &close_resp, glyph::CLOSE, true, &pal);

            if opt_resp.clicked() {
                let mut g = core.lock().unwrap();
                g.options_open = true;
                g.options_want_focus = true;
            }
            if min_resp.clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            if close_resp.clicked() {
                let mut g = core.lock().unwrap();
                g.main_visible = false;
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }

            // 2) Body Area (debajo del titlebar — cualquier modal oscurece SOLO esta zona)
            let body_rect = ui.available_rect_before_wrap();
            let mut card_anchor_rect = Rect::NOTHING;
            let mut sel_btn_rect = Rect::NOTHING;
            ui.scope_builder(egui::UiBuilder::new().max_rect(body_rect).layout(Layout::top_down(Align::Min)), |ui| {
                egui::Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                    let avail_w = ui.available_width();
                    let col_gap = 16.0;
                    let left_w = ((avail_w - col_gap) * (1.28 / 2.20)).floor();
                    let right_w = avail_w - col_gap - left_w;
                    let col_h = ui.available_height();

                    ui.horizontal_top(|ui| {
                        // Columna izquierda
                        ui.allocate_ui_with_layout(
                            Vec2::new(left_w, col_h),
                            Layout::top_down(Align::Min),
                            |ui| {
                                (card_anchor_rect, sel_btn_rect) = render_left_column(core, ui, &pal, &active_panel);
                            },
                        );
                        ui.add_space(col_gap);
                        // Columna derecha (Panel activo o vista QR)
                        ui.allocate_ui_with_layout(
                            Vec2::new(right_w, col_h),
                            Layout::top_down(Align::Min),
                            |ui| {
                                egui::Frame::new()
                                    .fill(pal.surface_card)
                                    .stroke(Stroke::new(1.0, pal.outline))
                                    .corner_radius(CornerRadius::same(16))
                                    .inner_margin(Margin::same(16))
                                    .show(ui, |ui| {
                                        ui.set_min_size(Vec2::new(right_w - 34.0, (col_h - 34.0).max(200.0)));
                                        if let Some(ref m) = qr_method {
                                            render_qr_panel(core, ui, &pal, m);
                                        } else {
                                            render_transport_panel(core, ui, &pal, &active_panel);
                                        }
                                    });
                            },
                        );
                    });
                });

                // Dropdown flotante de perfiles guardados (anclado exactamente bajo la tarjeta activa)
                render_device_dropdown_overlay(core, ui, &pal, card_anchor_rect, sel_btn_rect);

                // Modales de confirmación/aviso acotados SOLO al body (sin cubrir titlebar)
                render_body_modals(core, ui, &pal, body_rect);
            });
        });

    let _ = ctx;
}

fn render_left_column(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    active_panel: &str,
) -> (Rect, Rect) {
    let (
        header_count_txt,
        sec_mobile_lbl,
        sec_conn_lbl,
        sec_conn_hint,
        device_name,
        device_os,
        device_meta,
        led_color,
        transport_icon,
        has_dropdown,
        dropdown_open,
        is_connected,
        is_paused,
        pause_lbl,
        driver_online,
        tcp_enabled,
        bt_enabled,
        usb_enabled,
        relay_connected,
        wfd_active,
        lang,
    ) = {
        let g = core.lock().unwrap();
        let st = &g.live;
        let snap = &st.snapshot;
        let count_txt = if st.connected {
            if snap.trusted_devices.len() > 1 {
                g.t_vars("devices_1_connected_saved", &[("count", &snap.trusted_devices.len().to_string())])
            } else {
                g.t("devices_1_connected").to_string()
            }
        } else if snap.trusted_devices.len() == 1 {
            g.t("devices_n_saved_one").to_string()
        } else if snap.trusted_devices.len() > 1 {
            g.t_vars("devices_n_saved_many", &[("count", &snap.trusted_devices.len().to_string())])
        } else {
            g.t("devices_0_connected").to_string()
        };

        let active_profile = st
            .selected_device_id
            .as_deref()
            .and_then(|id| snap.trusted_devices.iter().find(|d| d.device_id == id))
            .or_else(|| snap.trusted_devices.first());

        let (d_name, d_os, d_meta, l_col, tr_ic) = if !st.driver_online {
            (
                g.t("device_driver_stopped").to_string(),
                "android".to_string(),
                g.t("device_driver_start_hint").to_string(),
                pal.red_disconnect,
                None,
            )
        } else if st.connected {
            let os = st.device_os.clone().unwrap_or_else(|| "android".to_string());
            let name = st.device_name.clone().unwrap_or_else(|| {
                g.t(if os == "ios" { "device_ios_fallback" } else { "device_android_fallback" }).to_string()
            });
            let meta = if st.paused {
                g.t("device_reader_paused").to_string()
            } else {
                g.t_vars("device_reader_active_ms", &[("ms", &st.latency_ms.to_string())])
            };
            let led = if st.paused { pal.amber_pause } else { pal.led_green };
            let tic = match st.transport.as_deref() {
                Some("usb") => Some(glyph::USB),
                Some("bluetooth") => Some(glyph::BLUETOOTH),
                Some("relay") => Some(glyph::LINK),
                Some("wifi_direct") => Some(glyph::WIFI_TETHERING),
                _ => Some(glyph::WIFI),
            };
            (name, os, meta, led, tic)
        } else if let Some(prof) = active_profile {
            let tic = match prof.transport.as_deref() {
                Some("usb") => Some(glyph::USB),
                Some("bluetooth") => Some(glyph::BLUETOOTH),
                Some("relay") => Some(glyph::LINK),
                Some("wifi_direct") => Some(glyph::WIFI_TETHERING),
                _ => Some(glyph::WIFI),
            };
            (
                prof.device_name.clone(),
                prof.device_os.clone().unwrap_or_else(|| "android".to_string()),
                g.t("device_saved_disconnected").to_string(),
                pal.red_disconnect,
                tic,
            )
        } else {
            (
                g.t("device_none_title").to_string(),
                "android".to_string(),
                g.t("device_none_subtitle").to_string(),
                pal.on_surface_muted,
                None,
            )
        };

        (
            count_txt,
            g.t("section_mobile_device").to_string(),
            g.t("section_connection_options").to_string(),
            g.t("connection_options_hint").to_string(),
            d_name,
            d_os,
            d_meta,
            l_col,
            tr_ic,
            !snap.trusted_devices.is_empty(),
            g.device_dropdown_open,
            st.connected,
            st.paused,
            g.t(if st.paused { "btn_resume" } else { "btn_pause" }).to_string(),
            st.driver_online,
            snap.tcp_enabled,
            snap.bt_enabled,
            snap.usb_enabled,
            snap.relay_connected,
            snap.wfd_enabled && snap.wfd_available,
            g.current_lang().to_string(),
        )
    };

    // Cabecera "Dispositivo móvil"
    ui.horizontal(|ui| {
        ui.label(RichText::new(sec_mobile_lbl).font(semibold(16.0)).color(pal.on_surface));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(header_count_txt).size(12.0).color(pal.on_surface_muted));
        });
    });
    ui.add_space(4.0);

    // Tarjeta Dispositivo Activo
    let mut sel_btn_rect = Rect::NOTHING;
    let card_resp = egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            let full_w = ui.available_width();
            ui.horizontal(|ui| {
                // Zona clicable para abrir dropdown de perfiles guardados
                let left_btn_w = (full_w - 122.0).max(150.0);
                let (sel_rect, sel_resp) = ui.allocate_exact_size(
                    Vec2::new(left_btn_w, 42.0),
                    if has_dropdown { Sense::click() } else { Sense::hover() },
                );
                sel_btn_rect = sel_rect;
                let hover_bg = if has_dropdown && sel_resp.hovered() {
                    ui.painter().rect_filled(sel_rect, CornerRadius::same(8), pal.surface_elevated);
                    true
                } else {
                    false
                };

                let icon_box = Rect::from_min_size(Pos2::new(sel_rect.left() + 2.0, sel_rect.top() + 1.0), Vec2::splat(40.0));
                let (ic_bg, ic_border, ic_fg) = if is_connected {
                    (pal.accent_soft, pal.accent_border, pal.accent)
                } else {
                    (pal.surface_elevated, pal.outline, pal.on_surface_muted)
                };
                ui.painter().rect(
                    icon_box,
                    CornerRadius::same(8),
                    ic_bg,
                    Stroke::new(1.0, ic_border),
                    StrokeKind::Inside,
                );
                if driver_online && (is_connected || has_dropdown) {
                    let eye_bg = if is_connected { pal.surface_card } else { pal.surface_elevated };
                    paint_device_os_icon(ui.painter(), icon_box.shrink(10.0), &device_os, ic_fg, eye_bg);
                } else {
                    paint_glyph(ui.painter(), icon_box.center(), glyph::PHONELINK_OFF, 18.0, pal.on_surface_muted);
                }
                let _ = hover_bg;

                let tx_x = icon_box.right() + 8.0;
                let name_gal = ui.painter().layout_no_wrap(
                    device_name,
                    semibold(16.0),
                    pal.on_surface,
                );
                let name_w = name_gal.size().x.min(left_btn_w - 114.0);
                ui.painter().galley(Pos2::new(tx_x, sel_rect.top() + 1.0), name_gal, pal.on_surface);

                let line1_cy = sel_rect.top() + 11.0;
                let mut badge_x = tx_x + name_w + 8.0;
                ui.painter().circle_filled(Pos2::new(badge_x, line1_cy), 4.0, led_color);
                badge_x += 10.0;

                if let Some(tic) = transport_icon {
                    let cap = Rect::from_center_size(Pos2::new(badge_x + 10.0, line1_cy), Vec2::new(24.0, 19.0));
                    let (cap_bg, cap_stroke, cap_fg) = if is_connected {
                        (pal.accent_soft, pal.accent_border, pal.accent_on_soft)
                    } else {
                        (pal.surface_elevated, pal.outline, pal.on_surface_muted)
                    };
                    ui.painter().rect(
                        cap,
                        CornerRadius::same(10),
                        cap_bg,
                        Stroke::new(1.0, cap_stroke),
                        StrokeKind::Inside,
                    );
                    paint_glyph(ui.painter(), cap.center(), tic, 13.0, cap_fg);
                    badge_x += 25.0;
                }
                if has_dropdown {
                    paint_chevron_y(
                        ui.painter(),
                        Pos2::new(badge_x + 7.0, line1_cy),
                        dropdown_open,
                        15.0,
                        pal.on_surface_muted,
                    );
                }

                ui.painter().text(
                    Pos2::new(tx_x, sel_rect.top() + 23.0),
                    egui::Align2::LEFT_TOP,
                    device_meta,
                    FontId::proportional(12.0),
                    pal.on_surface_muted,
                );

                if has_dropdown && sel_resp.clicked() {
                    let mut g = core.lock().unwrap();
                    g.device_dropdown_open = !g.device_dropdown_open;
                }

                ui.add_space(6.0);

                // Botón Pausar / Reanudar (h=32px, px=8px, opacity=0.35 cuando desactivado)
                let lbl_gal = ui.painter().layout_no_wrap(pause_lbl.clone(), FontId::proportional(12.0), pal.on_surface);
                let pause_btn_w = (lbl_gal.size().x + 38.0).clamp(74.0, 92.0);
                let (p_rect, p_resp) = ui.allocate_exact_size(
                    Vec2::new(pause_btn_w, 32.0),
                    if is_connected { Sense::click() } else { Sense::hover() },
                );
                let (p_bg, p_stroke, p_fg, p_ic) = if !is_connected {
                    (
                        pal.surface_elevated.gamma_multiply(0.35),
                        pal.outline.gamma_multiply(0.35),
                        pal.on_surface.gamma_multiply(0.35),
                        glyph::PAUSE,
                    )
                } else if is_paused {
                    (pal.amber_soft, pal.amber_pause, pal.on_amber_soft, glyph::PLAY_ARROW)
                } else {
                    (
                        if p_resp.hovered() { pal.outline } else { pal.surface_elevated },
                        pal.outline,
                        pal.on_surface,
                        glyph::PAUSE,
                    )
                };
                ui.painter().rect(p_rect, CornerRadius::same(8), p_bg, Stroke::new(1.0, p_stroke), StrokeKind::Inside);
                let p_ic_col = if is_connected && is_paused {
                    pal.amber_pause
                } else if is_connected {
                    pal.on_surface_muted
                } else {
                    pal.on_surface_muted.gamma_multiply(0.35)
                };
                paint_glyph(ui.painter(), Pos2::new(p_rect.left() + 14.0, p_rect.center().y), p_ic, 16.0, p_ic_col);
                ui.painter().text(
                    Pos2::new(p_rect.left() + 26.0, p_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    pause_lbl,
                    FontId::proportional(12.0),
                    p_fg,
                );
                if is_connected && p_resp.clicked() {
                    core.lock().unwrap().send_ipc(IpcCommand::SetPaused { paused: !is_paused });
                }

                ui.add_space(6.0);

                // Botón Desconectar (32x32, fondo transparente según HTML!)
                let (d_rect, d_resp) = ui.allocate_exact_size(
                    Vec2::splat(32.0),
                    if is_connected { Sense::click() } else { Sense::hover() },
                );
                let d_alpha = if is_connected { 1.0 } else { 0.35 };
                ui.painter().rect(
                    d_rect,
                    CornerRadius::same(8),
                    if is_connected && d_resp.hovered() { pal.red_soft } else { Color32::TRANSPARENT },
                    Stroke::new(1.0, pal.red_border.gamma_multiply(d_alpha)),
                    StrokeKind::Inside,
                );
                paint_glyph(
                    ui.painter(),
                    d_rect.center(),
                    glyph::LINK_OFF,
                    16.0,
                    pal.red_disconnect.gamma_multiply(d_alpha),
                );
                if is_connected && d_resp.clicked() {
                    core.lock().unwrap().send_ipc(IpcCommand::DisconnectDevice);
                }
            });
        });

    ui.add_space(12.0);
    paint_divider(ui, pal);
    ui.add_space(12.0);

    // Cabecera "Opciones de conexión"
    ui.horizontal(|ui| {
        ui.label(RichText::new(sec_conn_lbl).font(semibold(16.0)).color(pal.on_surface));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(sec_conn_hint).size(11.0).color(pal.on_surface_muted));
        });
    });
    ui.add_space(4.0);

    // 5 Tarjetas de conexión (sin caja cuadrada alrededor del icono, 8px radius, gap 6px)
    let rows = [
        ("usb", glyph::USB, "method_usb_title", "method_usb_desc", usb_enabled),
        ("tcp", glyph::WIFI, "method_tcp_title", "method_tcp_desc", tcp_enabled),
        ("bluetooth", glyph::BLUETOOTH, "method_bt_title", "method_bt_desc", bt_enabled),
        ("relay", glyph::LINK, "method_relay_title", "method_relay_desc", relay_connected || true),
        ("wifi_direct", glyph::WIFI_TETHERING, "method_wfd_title", "method_wfd_desc", wfd_active || true),
    ];

    let gap_y = 6.0;
    let card_h = ((ui.available_height() - gap_y * 4.0) / 5.0).clamp(48.0, 53.0);
    for (idx, (key, ic, tk, dk, _active_flag)) in rows.iter().enumerate() {
        let selected = active_panel == *key;
        let (title, desc) = {
            let g = core.lock().unwrap();
            (g.i18n.t(&lang, tk).to_string(), g.i18n.t(&lang, dk).to_string())
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), card_h), Sense::click());
        let (bg, border) = if selected {
            (
                if resp.hovered() { pal.accent_soft } else { pal.accent_card_bg },
                pal.accent_border,
            )
        } else {
            (
                if resp.hovered() { pal.surface_elevated } else { pal.surface_card },
                pal.outline,
            )
        };
        ui.painter().rect(rect, CornerRadius::same(8), bg, Stroke::new(1.0, border), StrokeKind::Inside);

        // Icono directo 18px en color acento (sin caja contenedora, igual que HTML!)
        paint_glyph(
            ui.painter(),
            Pos2::new(rect.left() + 21.0, rect.center().y),
            *ic,
            18.0,
            pal.accent,
        );

        let tx = rect.left() + 38.0;
        ui.painter().text(
            Pos2::new(tx, rect.top() + 8.0),
            egui::Align2::LEFT_TOP,
            title,
            semibold(13.5),
            pal.on_surface,
        );
        ui.painter().text(
            Pos2::new(tx, rect.top() + 27.0),
            egui::Align2::LEFT_TOP,
            desc,
            FontId::proportional(11.0),
            pal.on_surface_muted,
        );

        paint_glyph(
            ui.painter(),
            Pos2::new(rect.right() - 18.0, rect.center().y),
            glyph::CHEVRON_RIGHT,
            16.0,
            if selected { pal.accent } else { pal.on_surface_muted },
        );

        if resp.clicked() {
            let mut g = core.lock().unwrap();
            g.active_panel = (*key).to_string();
            g.qr_method = None;
            g.qr_error = None;
        }
        if idx < rows.len() - 1 {
            ui.add_space(gap_y);
        }
    }

    (card_resp.response.rect, sel_btn_rect)
}

// ==================== 2. TRANSPORT & QR RIGHT PANELS ====================

fn render_transport_panel(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    panel: &str,
) {
    match panel {
        "usb" => render_panel_usb(core, ui, pal),
        "tcp" => render_panel_tcp(core, ui, pal),
        "bluetooth" => render_panel_bt(core, ui, pal),
        "relay" => render_panel_relay(core, ui, pal),
        "wifi_direct" => render_panel_wfd(core, ui, pal),
        _ => render_panel_usb(core, ui, pal),
    }
}

fn render_panel_usb(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
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
            g.t(if usb_en { "panel_usb_native_active" } else { "panel_usb_native_off" }).to_string(),
            g.t("panel_usb_subhint").to_string(),
            g.t("panel_usb_auto_desc").to_string(),
            g.t("panel_usb_auto_label").to_string(),
            g.t(if snap.usb_auto_connect { "state_on" } else { "state_off" }).to_string(),
            snap.usb_auto_connect,
            usb_en,
            g.t(if usb_en { "panel_usb_listen_on" } else { "panel_usb_listen_off" }).to_string(),
        )
    };

    // Header (solo título + pill, igual que HTML justify-between)
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(16.0)).color(pal.on_surface));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            paint_status_badge(ui, &status_txt, status_on, pal);
        });
    });

    // Centrado vertical entre header y botones inferiores (flex-col justify-between)
    let footer_h = 80.0; // 36 + 4 + 40
    let body_h = (ui.available_height() - footer_h).max(120.0);
    let inner_h = 114.0;
    let top_pad = ((body_h - inner_h) * 0.5).max(12.0);
    ui.add_space(top_pad);

    egui::Frame::new()
        .fill(pal.surface_elevated)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                ui.painter().rect(
                    ic_r,
                    CornerRadius::same(8),
                    pal.accent_soft,
                    Stroke::new(1.0, pal.accent_border),
                    StrokeKind::Inside,
                );
                paint_glyph(ui.painter(), ic_r.center(), glyph::USB, 18.0, pal.accent);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(hero_title).font(semibold(14.0)).color(pal.on_surface));
                    ui.label(RichText::new(hero_sub).monospace().size(11.0).color(pal.on_surface_muted));
                });
            });
        });

    ui.add_space(12.0);
    ui.label(RichText::new(auto_desc).size(11.0).color(pal.on_surface_muted));

    let rem = ui.available_height();
    if rem > footer_h {
        ui.add_space(rem - footer_h);
    }

    if toggle_state_btn(ui, pal, if usb_auto { glyph::USB } else { glyph::USB_OFF }, &auto_lbl, &auto_on_lbl, usb_auto) {
        core.lock().unwrap().send_ipc(IpcCommand::SetUsbPriority { enabled: !usb_auto });
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::CABLE, &toggle_lbl, usb_enabled) {
        core.lock().unwrap().send_ipc(IpcCommand::SetUsbEnabled { enabled: !usb_enabled });
    }
}

fn render_panel_tcp(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (title, desc, status_txt, tcp_enabled, ip_val, port_val, ip_lbl, port_lbl, qr_lbl, toggle_lbl, state_lbl, copied_ip, copied_port) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let ip = snap.tcp_ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        let now = Instant::now();
        (
            g.t("method_tcp_title").to_string(),
            g.t("panel_tcp_desc").to_string(),
            g.t(if snap.tcp_enabled { "panel_tcp_status_listening" } else { "state_off" }).to_string(),
            snap.tcp_enabled,
            ip,
            snap.tcp_port.to_string(),
            g.t("panel_tcp_ip_label").to_string(),
            g.t("panel_tcp_port_label").to_string(),
            g.t("btn_generate_qr").to_string(),
            g.t(if snap.tcp_enabled { "panel_tcp_toggle_disable" } else { "panel_tcp_toggle_enable" }).to_string(),
            g.t(if snap.tcp_enabled { "panel_bt_status_active" } else { "state_off" }).to_string(),
            g.copied_key.as_ref().is_some_and(|(k, exp)| k == "tcp_ip" && now < *exp),
            g.copied_key.as_ref().is_some_and(|(k, exp)| k == "tcp_port" && now < *exp),
        )
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(16.0)).color(pal.on_surface));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            paint_status_badge(ui, &status_txt, tcp_enabled, pal);
        });
    });

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(110.0);
    let inner_h = 86.0;
    ui.add_space(((body_h - inner_h) * 0.5).max(12.0));

    // Grid 2 columnas 1.4fr : 0.8fr con gap 4px — igual que HTML Image 3!
    let full_w = ui.available_width();
    let gap = 4.0;
    let w_left = ((full_w - gap) * (1.4 / 2.2)).floor();
    let w_right = full_w - gap - w_left;
    ui.horizontal(|ui| {
        if info_copy_tile_w(ui, pal, w_left, &ip_lbl, &ip_val, copied_ip) {
            ui.ctx().copy_text(ip_val.clone());
            core.lock().unwrap().copied_key = Some(("tcp_ip".to_string(), Instant::now() + Duration::from_secs(2)));
        }
        ui.add_space(gap);
        if info_copy_tile_w(ui, pal, w_right, &port_lbl, &port_val, copied_port) {
            ui.ctx().copy_text(port_val.clone());
            core.lock().unwrap().copied_key = Some(("tcp_port".to_string(), Instant::now() + Duration::from_secs(2)));
        }
    });

    ui.add_space(12.0);
    ui.label(RichText::new(desc).size(11.0).color(pal.on_surface_muted));

    let rem = ui.available_height();
    if rem > footer_h {
        ui.add_space(rem - footer_h);
    }

    if toggle_state_btn(ui, pal, if tcp_enabled { glyph::WIFI } else { glyph::WIFI_OFF }, &toggle_lbl, &state_lbl, tcp_enabled) {
        core.lock().unwrap().send_ipc(IpcCommand::SetTcpEnabled { enabled: !tcp_enabled });
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::QR_CODE, &qr_lbl, true) {
        trigger_qr_request(core, ui.ctx(), "tcp");
    }
}

fn render_panel_bt(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (title, status_txt, bt_enabled, adapter_val, mac_val, adapter_lbl, mac_lbl, body_desc, qr_lbl, toggle_lbl, state_lbl) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let adapter = snap.bt_name.clone().unwrap_or_else(|| {
            if snap.pc_name.is_empty() { "Windows BT".to_string() } else { snap.pc_name.clone() }
        });
        let mac = snap.bt_mac.clone().unwrap_or_else(|| g.t("panel_bt_unavailable").to_string());
        let st_txt = if !snap.bt_available {
            g.t("panel_bt_no_radio")
        } else if !snap.bt_enabled {
            g.t("state_off")
        } else {
            g.t("panel_bt_status_active")
        };
        (
            g.t("method_bt_title").to_string(),
            st_txt.to_string(),
            snap.bt_enabled && snap.bt_available,
            adapter,
            mac,
            g.t("panel_bt_adapter_label").to_string(),
            g.t("panel_bt_mac_label").to_string(),
            g.t("panel_bt_desc_qr").to_string(),
            g.t("btn_generate_qr").to_string(),
            g.t(if snap.bt_enabled { "panel_bt_toggle_disable" } else { "panel_bt_toggle_enable" }).to_string(),
            g.t(if snap.bt_enabled { "panel_bt_status_active" } else { "state_off" }).to_string(),
        )
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(16.0)).color(pal.on_surface));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            paint_status_badge(ui, &status_txt, bt_enabled, pal);
        });
    });

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(120.0);
    let inner_h = 114.0;
    ui.add_space(((body_h - inner_h) * 0.5).max(12.0));

    info_kv_row(ui, pal, &adapter_lbl, &adapter_val, true);
    ui.add_space(8.0);
    info_kv_row(ui, pal, &mac_lbl, &mac_val, false);
    ui.add_space(10.0);
    ui.label(RichText::new(body_desc).size(11.0).color(pal.on_surface_muted));

    let rem = ui.available_height();
    if rem > footer_h {
        ui.add_space(rem - footer_h);
    }

    let bt_on_flag = { core.lock().unwrap().live.snapshot.bt_enabled };
    if toggle_state_btn(ui, pal, if bt_on_flag { glyph::BLUETOOTH } else { glyph::BLUETOOTH_DISABLED }, &toggle_lbl, &state_lbl, bt_on_flag) {
        core.lock().unwrap().send_ipc(IpcCommand::SetBluetoothEnabled { enabled: !bt_on_flag });
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::QR_CODE, &qr_lbl, true) {
        trigger_qr_request(core, ui.ctx(), "bluetooth");
    }
}

fn render_panel_relay(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (title, status_txt, status_on, url_lbl, save_lbl, ch_lbl, ch_val, link_lbl, live_link, renew_lbl, qr_lbl, relay_desc, copy_lbl, copied_link, has_url) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let has_u = snap.relay_url.as_deref().is_some_and(|u| !u.is_empty());
        let st_txt = if snap.relay_connected {
            g.t("panel_relay_status_connected")
        } else if has_u {
            g.t("panel_relay_status_reconnecting")
        } else {
            g.t("panel_relay_status_inactive")
        };
        let now = Instant::now();
        (
            g.t("method_relay_title").to_string(),
            st_txt.to_string(),
            snap.relay_connected,
            g.t("panel_relay_url_label").to_string(),
            g.t("btn_save").to_string(),
            g.t("panel_relay_channel_label").to_string(),
            snap.relay_channel.clone(),
            g.t("panel_relay_live_link_label").to_string(),
            g.live_relay_link.clone(),
            g.t("panel_relay_refresh_link").to_string(),
            g.t("btn_generate_qr").to_string(),
            g.t("panel_relay_desc").to_string(),
            g.t("btn_copy").to_string(),
            g.copied_key.as_ref().is_some_and(|(k, exp)| k == "relay_link" && now < *exp),
            has_u,
        )
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(16.0)).color(pal.on_surface));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            paint_status_badge(ui, &status_txt, status_on, pal);
        });
    });

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(150.0);
    let inner_h = if has_url && live_link.is_some() { 196.0 } else { 142.0 };
    ui.add_space(((body_h - inner_h) * 0.5).max(10.0));

    ui.label(RichText::new(url_lbl).size(11.0).color(pal.on_surface_muted));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let mut g = core.lock().unwrap();
        let w = (ui.available_width() - 80.0).max(120.0);
        styled_singleline_input(
            ui,
            pal,
            &mut g.relay_url_buf,
            w,
            36.0,
            "https://nfc-bridge.hectorzaga96.workers.dev",
        );
        ui.add_space(6.0);
        if small_secondary_btn(ui, pal, &save_lbl, 74.0, 36.0) {
            let trimmed = g.relay_url_buf.trim().to_string();
            let url_opt = if trimmed.is_empty() { None } else { Some(trimmed) };
            g.send_ipc(IpcCommand::SetRelayUrl { url: url_opt });
            g.live_relay_expires_at = None;
        }
    });

    ui.add_space(8.0);
    info_kv_row(ui, pal, &ch_lbl, if ch_val.is_empty() { "nb-..." } else { &ch_val }, false);

    if let Some(ref ll) = live_link {
        ui.add_space(8.0);
        let secs_left = core
            .lock()
            .unwrap()
            .live_relay_expires_at
            .map(|exp| exp.saturating_duration_since(Instant::now()).as_secs())
            .unwrap_or(90);
        let countdown = format!("{}:{:02}", secs_left / 60, secs_left % 60);
        let box_w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(Vec2::new(box_w, 48.0), Sense::hover());
        ui.painter().rect(
            r,
            CornerRadius::same(8),
            pal.surface_elevated,
            Stroke::new(1.0, pal.outline),
            StrokeKind::Inside,
        );
        // Fila superior: "Enlace temporal activo" + "1:29"
        ui.painter().text(
            Pos2::new(r.left() + 12.0, r.top() + 8.0),
            egui::Align2::LEFT_TOP,
            &link_lbl,
            FontId::proportional(11.0),
            pal.on_surface_muted,
        );
        ui.painter().text(
            Pos2::new(r.right() - 12.0, r.top() + 8.0),
            egui::Align2::RIGHT_TOP,
            countdown,
            FontId::monospace(11.0),
            pal.accent,
        );
        // Fila inferior: URL truncada antes de "Copiar" + botón texto "Copiar"/"Copiado"
        let copy_txt = if copied_link {
            core.lock().unwrap().t("btn_copied").to_string()
        } else {
            copy_lbl.clone()
        };
        let copy_gal = ui.painter().layout_no_wrap(copy_txt.clone(), FontId::proportional(11.0), pal.accent);
        let copy_rect = Rect::from_min_size(
            Pos2::new(r.right() - 12.0 - copy_gal.size().x, r.top() + 25.0),
            Vec2::new(copy_gal.size().x + 4.0, 16.0),
        );
        let copy_resp = ui.interact(copy_rect, Id::unique("relay_copy_link_btn"), Sense::click());
        let max_url_w = (copy_rect.left() - (r.left() + 12.0) - 10.0).max(60.0);
        let trunc_url = truncate_text_to_width(ui.painter(), ll, FontId::monospace(11.0), max_url_w);
        ui.painter().text(
            Pos2::new(r.left() + 12.0, r.top() + 26.0),
            egui::Align2::LEFT_TOP,
            trunc_url,
            FontId::monospace(11.0),
            pal.on_surface,
        );
        ui.painter().galley(
            Pos2::new(copy_rect.left(), r.top() + 26.0),
            copy_gal,
            pal.accent,
        );
        if copy_resp.hovered() {
            ui.painter().line_segment(
                [Pos2::new(copy_rect.left(), r.top() + 39.0), Pos2::new(copy_rect.right() - 4.0, r.top() + 39.0)],
                Stroke::new(1.0, pal.accent),
            );
        }
        if copy_resp.clicked() {
            ui.ctx().copy_text(ll.clone());
            core.lock().unwrap().copied_key = Some(("relay_link".to_string(), Instant::now() + Duration::from_secs(2)));
        }
    }

    ui.add_space(8.0);
    ui.label(RichText::new(relay_desc).size(11.0).color(pal.on_surface_muted));

    let rem = ui.available_height();
    if rem > footer_h {
        ui.add_space(rem - footer_h);
    }

    if secondary_btn_accent_icon(ui, pal, glyph::REFRESH, &renew_lbl) {
        core.lock().unwrap().live_relay_expires_at = None;
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::QR_CODE, &qr_lbl, true) {
        trigger_qr_request(core, ui.ctx(), "relay");
    }
}

fn render_panel_wfd(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (title, status_txt, wfd_active, wfd_enabled, wfd_state_lbl, ssid_val, pass_val, ip_val, ssid_lbl, pass_lbl, ip_lbl, toggle_lbl, qr_lbl, copied_ssid, copied_pass) = {
        let g = core.lock().unwrap();
        let snap = &g.live.snapshot;
        let st_txt = if !snap.wfd_enabled {
            g.t("panel_wfd_status_inactive")
        } else if snap.wfd_available {
            g.t("panel_wfd_status_active")
        } else {
            g.t("panel_wfd_status_no_wifi")
        };
        let state_r = if snap.wfd_enabled {
            g.t(if snap.wfd_available { "panel_wfd_state_open" } else { "panel_wfd_state_waiting" })
        } else {
            g.t("panel_wfd_state_closed")
        };
        let ssid = snap.wfd_ssid.clone().unwrap_or_else(|| {
            g.t(if snap.wfd_enabled { "panel_wfd_ssid_activating" } else { "panel_wfd_ssid_hint" }).to_string()
        });
        let pass = snap.wfd_passphrase.clone().unwrap_or_else(|| "--".to_string());
        let ip = format!("{}:{}", snap.wfd_ip.as_deref().unwrap_or("192.168.137.1"), snap.tcp_port);
        let now = Instant::now();
        (
            g.t("method_wfd_title").to_string(),
            st_txt.to_string(),
            snap.wfd_enabled && snap.wfd_available,
            snap.wfd_enabled,
            state_r.to_string(),
            ssid,
            pass,
            ip,
            g.t("panel_wfd_ssid_label").to_string(),
            g.t("panel_wfd_pass_label").to_string(),
            g.t("panel_wfd_ip_label").to_string(),
            g.t(if snap.wfd_enabled { "panel_wfd_close" } else { "panel_wfd_open" }).to_string(),
            g.t("btn_generate_qr").to_string(),
            g.copied_key.as_ref().is_some_and(|(k, exp)| k == "wfd_ssid" && now < *exp),
            g.copied_key.as_ref().is_some_and(|(k, exp)| k == "wfd_pass" && now < *exp),
        )
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(16.0)).color(pal.on_surface));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            paint_status_badge(ui, &status_txt, wfd_active, pal);
        });
    });

    let footer_h = 80.0;
    let body_h = (ui.available_height() - footer_h).max(140.0);
    let inner_h = 152.0;
    ui.add_space(((body_h - inner_h) * 0.5).max(10.0));

    if info_copy_tile_w_col(ui, pal, ui.available_width(), &ssid_lbl, &ssid_val, copied_ssid, pal.on_surface) {
        ui.ctx().copy_text(ssid_val.clone());
        core.lock().unwrap().copied_key = Some(("wfd_ssid".to_string(), Instant::now() + Duration::from_secs(2)));
    }
    ui.add_space(8.0);
    if info_copy_tile_w_col(ui, pal, ui.available_width(), &pass_lbl, &pass_val, copied_pass, pal.accent) {
        ui.ctx().copy_text(pass_val.clone());
        core.lock().unwrap().copied_key = Some(("wfd_pass".to_string(), Instant::now() + Duration::from_secs(2)));
    }
    ui.add_space(8.0);
    info_kv_row(ui, pal, &ip_lbl, &ip_val, false);

    let rem = ui.available_height();
    if rem > footer_h {
        ui.add_space(rem - footer_h);
    }

    if toggle_state_btn(ui, pal, glyph::WIFI_TETHERING, &toggle_lbl, &wfd_state_lbl, wfd_enabled) {
        core.lock().unwrap().send_ipc(IpcCommand::SetWifiDirectEnabled { enabled: !wfd_enabled });
    }
    ui.add_space(4.0);
    if primary_cta_btn(ui, pal, glyph::QR_CODE, &qr_lbl, true) {
        trigger_qr_request(core, ui.ctx(), "wifi_direct");
    }
}

fn render_qr_panel(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    method: &str,
) {
    let (back_lbl, method_lbl, loading, err, qr_opt, copy_temp_lbl, copied_temp, refresh_lbl, generating_lbl) = {
        let g = core.lock().unwrap();
        let m_lbl = match method {
            "tcp" => g.t("qr_method_tcp"),
            "bluetooth" => g.t("qr_method_bluetooth"),
            "wifi_direct" => g.t("qr_method_wifi_direct"),
            _ => g.t("qr_method_relay"),
        };
        let now = Instant::now();
        (
            g.t("qr_back").to_string(),
            m_lbl.to_string(),
            g.qr_loading,
            g.qr_error.clone(),
            g.qr_data.clone(),
            g.t("qr_copy_temp_link").to_string(),
            g.copied_key.as_ref().is_some_and(|(k, exp)| k == "qr_share" && now < *exp),
            g.t("qr_refresh").to_string(),
            g.t("qr_generating").to_string(),
        )
    };

    ui.horizontal(|ui| {
        if small_ghost_btn(ui, pal, glyph::ARROW_BACK, &back_lbl) {
            let mut g = core.lock().unwrap();
            g.qr_method = None;
            g.qr_error = None;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            paint_status_badge(ui, &method_lbl, true, pal);
        });
    });

    let has_share = qr_opt.as_ref().is_some_and(|q| q.share_link.is_some());
    let bottom_block_h = if qr_opt.is_some() {
        if has_share { 112.0 } else { 76.0 }
    } else {
        36.0
    };
    let mid_space = (ui.available_height() - bottom_block_h - 192.0).max(16.0);
    ui.add_space((mid_space * 0.5).clamp(6.0, 22.0));

    // Caja QR centrada (192x192 redondeada — estilo Rust preferido, espacios fieles al HTML)
    ui.vertical_centered(|ui| {
        let (qr_rect, _) = ui.allocate_exact_size(Vec2::splat(192.0), Sense::hover());
        if loading {
            ui.painter().rect_filled(qr_rect, CornerRadius::same(12), Color32::WHITE);
            ui.painter().text(
                qr_rect.center(),
                egui::Align2::CENTER_CENTER,
                generating_lbl,
                FontId::proportional(12.0),
                Color32::from_rgb(18, 18, 18),
            );
        } else if let Some(ref e) = err {
            ui.painter().rect_filled(qr_rect, CornerRadius::same(12), Color32::WHITE);
            ui.painter().text(
                qr_rect.center(),
                egui::Align2::CENTER_CENTER,
                e,
                FontId::proportional(11.0),
                Color32::from_rgb(127, 41, 41),
            );
        } else if let Some(ref qr) = qr_opt {
            if let Err(e) = paint_qr_box(ui.painter(), qr_rect, &qr.uri, &qr.method, pal.accent) {
                ui.painter().text(qr_rect.center(), egui::Align2::CENTER_CENTER, e, FontId::proportional(11.0), pal.red_disconnect);
            }
        }
    });

    let rem_below_qr = ui.available_height();
    let needed_below = bottom_block_h;
    if rem_below_qr > needed_below + 6.0 {
        ui.add_space(((rem_below_qr - needed_below) * 0.5).clamp(6.0, 18.0));
    } else {
        ui.add_space(6.0);
    }

    if let Some(ref qr) = qr_opt {
        let secs_left = qr.expires_at.saturating_duration_since(Instant::now()).as_secs();
        let mins = secs_left / 60;
        let secs = secs_left % 60;
        let countdown = format!("{}:{:02}", mins, secs);

        egui::Frame::new()
            .fill(pal.surface_elevated)
            .stroke(Stroke::new(1.0, pal.outline))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&qr.pair_token).monospace().size(12.0).color(pal.on_surface));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(countdown).monospace().size(11.0).color(pal.accent));
                    });
                });
            });

        if let Some(ref share) = qr.share_link {
            ui.add_space(4.0);
            let btn_lbl = if copied_temp {
                core.lock().unwrap().t("qr_link_copied").to_string()
            } else {
                copy_temp_lbl
            };
            if secondary_btn(ui, pal, glyph::CONTENT_COPY, &btn_lbl) {
                ui.ctx().copy_text(share.clone());
                core.lock().unwrap().copied_key = Some(("qr_share".to_string(), Instant::now() + Duration::from_secs(2)));
            }
        }
    }

    let avail_h = ui.available_height();
    if avail_h > 36.0 {
        ui.add_space(avail_h - 36.0);
    }
    if secondary_btn(ui, pal, glyph::REFRESH, &refresh_lbl) {
        trigger_qr_request(core, ui.ctx(), method);
    }
}

fn trigger_qr_request(core: &Arc<Mutex<SharedUiCore>>, ctx: &egui::Context, method: &str) {
    let m = method.to_string();
    let (ipc, rt) = {
        let mut g = core.lock().unwrap();
        g.qr_method = Some(m.clone());
        g.qr_loading = true;
        g.qr_error = None;
        (g.ipc.clone(), g.rt.clone())
    };
    let core_c = core.clone();
    let ctx_c = ctx.clone();
    rt.spawn(async move {
        match ipc.request_pairing_qr(&m).await {
            Ok(data) => {
                let mut g = core_c.lock().unwrap();
                g.qr_loading = false;
                g.qr_data = Some(IssuedQrView {
                    method: data.method,
                    pair_token: data.pair_token,
                    expires_at: Instant::now() + Duration::from_secs(data.expires_in_secs.max(10)),
                    uri: data.uri,
                    share_link: data.share_link,
                });
            }
            Err(e) => {
                let mut g = core_c.lock().unwrap();
                g.qr_loading = false;
                g.qr_error = Some(e.to_string());
            }
        }
        ctx_c.request_repaint();
    });
}

// ==================== 3. SAVED DEVICES DROPDOWN & BODY MODALS ====================

fn render_device_dropdown_overlay(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    card_anchor_rect: Rect,
    sel_btn_rect: Rect,
) {
    let (
        is_open,
        devices,
        active_id,
        selected_id,
        connected_now,
        paused_now,
        latency_now,
        editing_id,
        mut editing_name,
        empty_lbl,
        ph_lbl,
        short_disconn,
        short_paused,
    ) = {
        let g = core.lock().unwrap();
        if !g.device_dropdown_open {
            return;
        }
        (
            g.device_dropdown_open,
            g.live.snapshot.trusted_devices.clone(),
            g.live.active_device_id.clone(),
            g.live.selected_device_id.clone(),
            g.live.connected,
            g.live.paused,
            g.live.latency_ms,
            g.editing_device_id.clone(),
            g.editing_device_name.clone(),
            g.t("device_dropdown_empty").to_string(),
            g.t("device_name_placeholder").to_string(),
            g.t("device_disconnected_short").to_string(),
            g.t("device_paused_short").to_string(),
        )
    };
    let _ = is_open;

    if !card_anchor_rect.is_positive() {
        return;
    }

    let inner_w = (card_anchor_rect.width() - 14.0).max(220.0);
    let dd_pos = Pos2::new(card_anchor_rect.left(), card_anchor_rect.bottom() + 4.0);

    let area_resp = egui::Area::new(Id::unique("nb_device_dropdown_area"))
        .order(egui::Order::Foreground)
        .fixed_pos(dd_pos)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(pal.surface_card)
                .stroke(Stroke::new(1.0, pal.outline))
                .corner_radius(CornerRadius::same(16))
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
                        .max_width(inner_w)
                        .max_height(240.0)
                        .show(ui, |ui| {
                            ui.set_min_width(inner_w);
                            ui.set_max_width(inner_w);
                            if devices.is_empty() {
                                ui.label(RichText::new(empty_lbl).size(12.0).color(pal.on_surface_muted));
                                return;
                            }
                            for (idx, dev) in devices.iter().enumerate() {
                                let is_active = connected_now && Some(&dev.device_id) == active_id.as_ref();
                                let is_selected_saved = !connected_now
                                    && (Some(&dev.device_id) == selected_id.as_ref()
                                        || (selected_id.is_none() && idx == 0));
                                let is_editing = editing_id.as_deref() == Some(dev.device_id.as_str());
                                let show_check = is_active || is_selected_saved;

                                // Rect exacto de la fila (48px alto) para hover instantáneo en toda la fila
                                let row_outer_rect = Rect::from_min_size(ui.cursor().min, Vec2::new(inner_w, 48.0));
                                let pointer_over_row = is_editing
                                    || ui.ctx().pointer_hover_pos().is_some_and(|p| row_outer_rect.contains(p));

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
                                // 1. LED
                                let dot_c = if is_active {
                                    if paused_now { pal.amber_pause } else { pal.led_green }
                                } else {
                                    pal.red_disconnect
                                };
                                ui.painter().circle_filled(Pos2::new(content_r.left() + 4.0, content_r.center().y), 4.0, dot_c);

                                // 2. Caja transporte 32x32
                                let tr_box = Rect::from_min_size(Pos2::new(content_r.left() + 18.0, content_r.top()), Vec2::splat(32.0));
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
                                let tr_ic = match dev.transport.as_deref() {
                                    Some("usb") => glyph::USB,
                                    Some("bluetooth") => glyph::BLUETOOTH,
                                    Some("relay") => glyph::LINK,
                                    Some("wifi_direct") => glyph::WIFI_TETHERING,
                                    _ => glyph::WIFI,
                                };
                                paint_glyph(ui.painter(), tr_box.center(), tr_ic, 18.0, tr_fg);

                                // 3. Zona derecha alineada fijamente en todas las filas (Delete, Edit, Check)
                                let dl_r = Rect::from_min_size(Pos2::new(content_r.right() - 28.0, content_r.center().y - 14.0), Vec2::splat(28.0));
                                let ed_r = Rect::from_min_size(Pos2::new(dl_r.left() - 32.0, content_r.center().y - 14.0), Vec2::splat(28.0));
                                let chk_center_x = if pointer_over_row {
                                    ed_r.left() - 14.0
                                } else {
                                    content_r.right() - 10.0
                                };

                                if is_editing {
                                    let edit_left = tr_box.right() + 10.0;
                                    let edit_w = (ed_r.left() - 40.0 - edit_left).max(70.0);
                                    let edit_rect = Rect::from_min_size(Pos2::new(edit_left, content_r.top()), Vec2::new(edit_w, 28.0));
                                    let mut enter_pressed = false;
                                    ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                                        let te = styled_singleline_input(ui, pal, &mut editing_name, edit_w, 28.0, &ph_lbl);
                                        enter_pressed = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    });
                                    core.lock().unwrap().editing_device_name = editing_name.clone();

                                    let save_r = Rect::from_min_size(Pos2::new(edit_rect.right() + 4.0, content_r.top()), Vec2::splat(28.0));
                                    let save_resp = ui.interact(save_r, Id::unique(("dd_save", &dev.device_id)), Sense::click());
                                    ui.painter().rect_filled(save_r, CornerRadius::same(8), pal.accent);
                                    paint_glyph(ui.painter(), save_r.center(), glyph::CHECK, 16.0, pal.on_accent);
                                    if save_resp.clicked() || enter_pressed {
                                        let clean = editing_name.trim().to_string();
                                        let mut g = core.lock().unwrap();
                                        if !clean.is_empty() {
                                            let id = dev.device_id.clone();
                                            g.rt.spawn({
                                                let ipc = g.ipc.clone();
                                                let id_c = id.clone();
                                                let nm_c = clean.clone();
                                                async move {
                                                    ipc.rename_saved_profile(&id_c, &nm_c).await;
                                                    let _ = ipc.send_command(IpcCommand::RenameTrustedDevice { device_id: id_c, name: nm_c }).await;
                                                }
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
                                    let click_rect = Rect::from_min_max(row_outer_rect.min, Pos2::new(if pointer_over_row { ed_r.left() - 4.0 } else { row_outer_rect.right() }, row_outer_rect.bottom()));
                                    let row_resp = ui.interact(click_rect, Id::unique(("dd_row", &dev.device_id)), Sense::click());

                                    let os_r = Rect::from_min_size(Pos2::new(text_left, content_r.top() + 2.0), Vec2::splat(14.0));
                                    let os_col = if is_active { pal.on_surface } else { pal.on_surface_muted };
                                    let os_eye_bg = if is_selected_saved || pointer_over_row { pal.surface_elevated } else { pal.surface_card };
                                    paint_device_os_icon(ui.painter(), os_r, dev.device_os.as_deref().unwrap_or("android"), os_col, os_eye_bg);

                                    let max_name_w = (right_limit - (text_left + 20.0)).max(50.0);
                                    let trunc_name = truncate_text_to_width(ui.painter(), &dev.device_name, semibold(14.0), max_name_w);
                                    ui.painter().text(Pos2::new(text_left + 20.0, content_r.top()), egui::Align2::LEFT_TOP, trunc_name, semibold(14.0), pal.on_surface);

                                    let raw_id = dev.mobile_id.as_deref().filter(|s| !s.is_empty()).unwrap_or(&dev.device_id);
                                    let st_part = if is_active {
                                        if paused_now { short_paused.as_str().to_string() } else { format!("{} ms", latency_now.max(1)) }
                                    } else {
                                        short_disconn.clone()
                                    };
                                    let sub_txt = format!("ID {} • {}", raw_id, st_part);
                                    let trunc_sub = truncate_text_to_width(ui.painter(), &sub_txt, FontId::monospace(10.0), (right_limit - text_left).max(50.0));
                                    ui.painter().text(Pos2::new(text_left, content_r.top() + 18.0), egui::Align2::LEFT_TOP, trunc_sub, FontId::monospace(10.0), pal.on_surface_muted);

                                    if row_resp.clicked() {
                                        let id = dev.device_id.clone();
                                        let mut g = core.lock().unwrap();
                                        g.device_dropdown_open = false;
                                        let ipc = g.ipc.clone();
                                        g.rt.spawn(async move {
                                            ipc.select_saved_profile(&id).await;
                                            let _ = ipc.send_command(IpcCommand::SelectTrustedDevice { device_id: id }).await;
                                        });
                                    }

                                    if show_check {
                                        paint_glyph(
                                            ui.painter(),
                                            Pos2::new(chk_center_x, content_r.center().y),
                                            glyph::CHECK,
                                            16.0,
                                            if is_active { pal.accent } else { pal.on_surface_muted },
                                        );
                                    }
                                }

                                // 4. Botones Editar / Eliminar visibles SOLO cuando el mouse está sobre esta fila (o en edición)
                                if pointer_over_row {
                                    let ed_resp = ui.interact(ed_r, Id::unique(("dd_edit", &dev.device_id)), Sense::click());
                                    ui.painter().rect(
                                        ed_r,
                                        CornerRadius::same(8),
                                        Color32::TRANSPARENT,
                                        Stroke::new(1.0, if ed_resp.hovered() { pal.accent } else { pal.outline }),
                                        StrokeKind::Inside,
                                    );
                                    paint_glyph(
                                        ui.painter(),
                                        ed_r.center(),
                                        glyph::EDIT,
                                        15.0,
                                        if ed_resp.hovered() { pal.accent } else { pal.on_surface_muted },
                                    );
                                    if ed_resp.clicked() {
                                        let mut g = core.lock().unwrap();
                                        g.editing_device_id = if is_editing { None } else { Some(dev.device_id.clone()) };
                                        g.editing_device_name = dev.device_name.clone();
                                    }

                                    let dl_resp = ui.interact(dl_r, Id::unique(("dd_del", &dev.device_id)), Sense::click());
                                    ui.painter().rect(
                                        dl_r,
                                        CornerRadius::same(8),
                                        if dl_resp.hovered() { pal.red_soft } else { Color32::TRANSPARENT },
                                        Stroke::new(1.0, if dl_resp.hovered() { pal.red_border } else { pal.outline }),
                                        StrokeKind::Inside,
                                    );
                                    paint_glyph(
                                        ui.painter(),
                                        dl_r.center(),
                                        glyph::DELETE,
                                        15.0,
                                        if dl_resp.hovered() { pal.red_disconnect } else { pal.on_surface_muted },
                                    );
                                    if dl_resp.clicked() {
                                        let id = dev.device_id.clone();
                                        let g = core.lock().unwrap();
                                        let ipc = g.ipc.clone();
                                        g.rt.spawn(async move {
                                            ipc.remove_saved_profile(&id).await;
                                            let _ = ipc.send_command(IpcCommand::RevokeTrustedDevice { device_id: id }).await;
                                        });
                                    }
                                }

                                // Avanzar cursor exactamente la altura de esta fila
                                ui.allocate_exact_size(Vec2::new(inner_w, 48.0), Sense::hover());
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

fn render_body_modals(
    core: &Arc<Mutex<SharedUiCore>>,
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    body_rect: Rect,
) {
    #[derive(Clone)]
    enum ActiveModal {
        MobileSwitch { req_id: String, title: String, desc: String, reject: String, accept: String },
        InvalidPairing { dev_id: Option<String>, title: String, desc: String, keep: String, delete: String },
        RestartReady { title: String, desc: String, later: String, now_lbl: String },
    }

    let modal = {
        let g = core.lock().unwrap();
        let app_name = if !g.updates.app_display_name.is_empty() {
            g.updates.app_display_name.clone()
        } else {
            g.t("app_sync_name").to_string()
        };
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
                    &[("name", &sw.device_name), ("transport", tr_lbl), ("app", &app_name)],
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

    let Some(m) = modal else { return; };

    // Fondo oscurecido acotado SOLO a body_rect (barra superior y botones quedan libres)
    egui::Area::new(Id::unique("nb_body_modal_scrim"))
        .order(egui::Order::Foreground)
        .fixed_pos(body_rect.min)
        .show(ui.ctx(), |ui| {
            ui.painter().rect_filled(body_rect, CornerRadius::ZERO, Color32::from_black_alpha(158));
            let card_rect = Rect::from_center_size(body_rect.center(), Vec2::new(390.0, 168.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(card_rect), |ui| {
                egui::Frame::new()
                    .fill(pal.surface_card)
                    .stroke(Stroke::new(1.0, pal.outline))
                    .corner_radius(CornerRadius::same(16))
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_min_size(Vec2::new(358.0, 136.0));
                        match m {
                            ActiveModal::MobileSwitch { req_id, title, desc, reject, accept } => {
                                ui.horizontal(|ui| {
                                    ui.label(crate::egui_icons::icon_rich(glyph::SWAP_HORIZ, 18.0, pal.accent));
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(title).size(15.0).strong().color(pal.on_surface));
                                });
                                ui.add_space(8.0);
                                ui.label(RichText::new(desc).size(12.5).color(pal.on_surface_muted));
                                ui.add_space(16.0);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if small_accent_btn(ui, pal, &accept, 120.0, 32.0) {
                                        let g = core.lock().unwrap();
                                        let ipc = g.ipc.clone();
                                        let rid = req_id.clone();
                                        g.rt.spawn(async move {
                                            ipc.clear_pending_mobile_switch(Some(&rid)).await;
                                            let _ = ipc.send_command(IpcCommand::RespondMobileSwitch { request_id: rid, accept: true }).await;
                                        });
                                    }
                                    ui.add_space(8.0);
                                    if small_secondary_btn(ui, pal, &reject, 96.0, 32.0) {
                                        let g = core.lock().unwrap();
                                        let ipc = g.ipc.clone();
                                        let rid = req_id.clone();
                                        g.rt.spawn(async move {
                                            ipc.clear_pending_mobile_switch(Some(&rid)).await;
                                            let _ = ipc.send_command(IpcCommand::RespondMobileSwitch { request_id: rid, accept: false }).await;
                                        });
                                    }
                                });
                            }
                            ActiveModal::InvalidPairing { dev_id, title, desc, keep, delete } => {
                                ui.horizontal(|ui| {
                                    ui.label(crate::egui_icons::icon_rich(glyph::WARNING, 18.0, pal.red_disconnect));
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(title).size(15.0).strong().color(pal.on_surface));
                                });
                                ui.add_space(8.0);
                                ui.label(RichText::new(desc).size(12.5).color(pal.on_surface_muted));
                                ui.add_space(16.0);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if small_danger_btn(ui, pal, &delete, 118.0, 32.0) {
                                        let g = core.lock().unwrap();
                                        let ipc = g.ipc.clone();
                                        g.rt.spawn(async move {
                                            ipc.clear_invalid_pairing_notice().await;
                                            if let Some(id) = dev_id {
                                                ipc.remove_saved_profile(&id).await;
                                                let _ = ipc.send_command(IpcCommand::RevokeTrustedDevice { device_id: id }).await;
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
                            ActiveModal::RestartReady { title, desc, later, now_lbl } => {
                                ui.horizontal(|ui| {
                                    ui.label(crate::egui_icons::icon_rich(glyph::SYSTEM_UPDATE_ALT, 18.0, pal.accent));
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(title).size(15.0).strong().color(pal.on_surface));
                                });
                                ui.add_space(8.0);
                                ui.label(RichText::new(desc).size(12.5).color(pal.on_surface_muted));
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

// ==================== 4. OPTIONS VIEWPORT (INDEPENDENT WINDOW) ====================

fn render_options_viewport(core: &Arc<Mutex<SharedUiCore>>, opt_title: &str, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    if ctx.input(|i| i.viewport().close_requested()) {
        core.lock().unwrap().options_open = false;
    }

    let pal = {
        let mut g = core.lock().unwrap();
        let pal = g.palette();
        let dwm_key = (pal.is_dark, pal.outline);
        if g.last_dwm_opts_key != Some(dwm_key) {
            g.last_dwm_opts_key = Some(dwm_key);
            apply_win32_dwm_to_title(opt_title, &pal);
        }
        pal
    };

    egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // Custom Options Title Bar (44 px)
            let (tb_rect, tb_resp) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click_and_drag());
            ui.painter().rect_filled(tb_rect, CornerRadius::ZERO, pal.surface_card);
            ui.painter().line_segment(
                [tb_rect.left_bottom(), tb_rect.right_bottom()],
                Stroke::new(1.0, pal.outline),
            );

            paint_glyph(ui.painter(), Pos2::new(tb_rect.left() + 22.0, tb_rect.center().y), glyph::TUNE, 17.0, pal.accent);
            let title_lbl = {
                let g = core.lock().unwrap();
                if g.updates.app_product == "tools" {
                    g.t("title_options_tools").to_string()
                } else {
                    g.t("title_options_sync").to_string()
                }
            };
            ui.painter().text(
                Pos2::new(tb_rect.left() + 38.0, tb_rect.center().y),
                egui::Align2::LEFT_CENTER,
                title_lbl,
                semibold(16.0),
                pal.on_surface,
            );

            let right = tb_rect.right() - 12.0;
            let cy = tb_rect.center().y;
            let close_r = Rect::from_center_size(Pos2::new(right - 14.0, cy), Vec2::splat(28.0));
            let min_r = Rect::from_center_size(Pos2::new(close_r.left() - 18.0, cy), Vec2::splat(28.0));
            let min_resp = ui.interact(min_r, Id::unique("opt_min"), Sense::click());
            let close_resp = ui.interact(close_r, Id::unique("opt_close"), Sense::click());

            if tb_resp.drag_started() && !min_resp.hovered() && !close_resp.hovered() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            paint_chrome_btn(ui.painter(), min_r, &min_resp, glyph::REMOVE, false, &pal);
            paint_chrome_btn(ui.painter(), close_r, &close_resp, glyph::CLOSE, true, &pal);
            if min_resp.clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            if close_resp.clicked() {
                core.lock().unwrap().options_open = false;
            }

            // Body: Collapsible Left Sidebar + Category Content
            let body_h = ui.available_height();
            let (compact_now, active_tab, has_any_update) = {
                let g = core.lock().unwrap();
                let has_u = g.updates.app_status == "available"
                    || g.updates.app_status == "installed_restart"
                    || g.updates.driver_status == "available";
                (g.sidebar_compact, g.options_tab, has_u)
            };

            let t_anim = ctx.animate_bool_with_time(Id::unique("nb_opts_sb_anim"), compact_now, 0.16);
            let sb_w = egui::lerp(196.0..=52.0, t_anim);

            ui.horizontal_top(|ui| {
                // Sidebar
                ui.allocate_ui_with_layout(Vec2::new(sb_w, body_h), Layout::top_down(Align::Min), |ui| {
                    let sb_rect = ui.available_rect_before_wrap();
                    ui.painter().rect_filled(sb_rect, CornerRadius::ZERO, pal.surface);
                    ui.painter().line_segment([sb_rect.right_top(), sb_rect.right_bottom()], Stroke::new(1.0, pal.outline));
                    ui.set_clip_rect(sb_rect);

                    egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
                        // Botón colapsar (anclado exactamente a x = left + 9 px como los iconos de categoría!)
                        let tog_rect = Rect::from_min_size(ui.cursor().min, Vec2::new((sb_w - 16.0).max(36.0), 32.0));
                        let tog_resp = ui.allocate_rect(tog_rect, Sense::click());
                        if tog_resp.hovered() {
                            ui.painter().rect_filled(tog_rect, CornerRadius::same(8), pal.surface_elevated);
                        }
                        paint_glyph(
                            ui.painter(),
                            Pos2::new(tog_rect.left() + 18.0, tog_rect.center().y),
                            if compact_now { glyph::CHEVRON_RIGHT } else { glyph::DOCK_TO_LEFT },
                            18.0,
                            pal.on_surface_muted,
                        );
                        if tog_resp.clicked() {
                            let mut g = core.lock().unwrap();
                            g.sidebar_compact = !g.sidebar_compact;
                            g.live.ui_settings.settings_sidebar_compact = g.sidebar_compact;
                            g.persist_ui_settings();
                        }

                        ui.add_space(6.0);

                        let nav_items = [
                            (OptionsTab::Appearance, glyph::PALETTE, "nav_appearance", false),
                            (OptionsTab::Network, glyph::LAN, "nav_network", false),
                            (OptionsTab::Updates, glyph::SYSTEM_UPDATE_ALT, "nav_updates", has_any_update),
                            (OptionsTab::About, glyph::INFO, "nav_about", false),
                        ];

                        for (tab_id, ic, key, show_dot) in nav_items {
                            let lbl = { core.lock().unwrap().t(key).to_string() };
                            let selected = active_tab == tab_id;
                            let btn_rect = Rect::from_min_size(
                                ui.cursor().min,
                                Vec2::new((sb_w - 16.0).max(36.0), 36.0),
                            );
                            let resp = ui.allocate_rect(btn_rect, Sense::click());
                            let (bg, border, fg) = if selected {
                                (pal.accent_soft, pal.accent_border, pal.accent_on_soft)
                            } else if resp.hovered() {
                                (pal.surface_elevated, Color32::TRANSPARENT, pal.on_surface)
                            } else {
                                (Color32::TRANSPARENT, Color32::TRANSPARENT, pal.on_surface_muted)
                            };
                            ui.painter().rect(btn_rect, CornerRadius::same(8), bg, Stroke::new(1.0, border), StrokeKind::Inside);

                            // Icono fijo en x = btn_rect.left() + 18.0 (cero salto horizontal al colapsar)
                            paint_glyph(
                                ui.painter(),
                                Pos2::new(btn_rect.left() + 18.0, btn_rect.center().y),
                                ic,
                                18.0,
                                fg,
                            );

                            let label_alpha = (1.0 - t_anim * 1.4).clamp(0.0, 1.0);
                            if label_alpha > 0.02 {
                                ui.painter().text(
                                    Pos2::new(btn_rect.left() + 36.0, btn_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    lbl,
                                    FontId::proportional(12.5),
                                    fg.gamma_multiply(label_alpha),
                                );
                            }
                            if show_dot {
                                let dot_pos = if compact_now {
                                    Pos2::new(btn_rect.right() - 7.0, btn_rect.top() + 7.0)
                                } else {
                                    Pos2::new(btn_rect.right() - 10.0, btn_rect.center().y)
                                };
                                ui.painter().circle_filled(dot_pos, 4.0, pal.accent);
                            }
                            if resp.clicked() {
                                core.lock().unwrap().options_tab = tab_id;
                            }
                            ui.add_space(4.0);
                        }
                    });
                });

                // Contenido principal de la categoría activa
                let content_w = (ui.available_width()).max(200.0);
                ui.allocate_ui_with_layout(Vec2::new(content_w, body_h), Layout::top_down(Align::Min), |ui| {
                    egui::Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            match active_tab {
                                OptionsTab::Appearance => render_opt_appearance(core, ui, &pal),
                                OptionsTab::Network => render_opt_network(core, ui, &pal),
                                OptionsTab::Updates => render_opt_updates(core, ui, &pal),
                                OptionsTab::About => render_opt_about(core, ui, &pal),
                            }
                        });
                    });
                });
            });

            // Selector HSV flotante anclado al clic (SIN oscurecer/difuminar el fondo)
            render_hsv_color_popover(core, ui, &pal);
        });
}

fn render_opt_appearance(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (hdr, theme_lbl, lang_lbl, win_acc_title, win_acc_desc, custom_lbl, theme_mode, lang_val, win_acc_on, win_hex, custom_hex, lang_open) = {
        let g = core.lock().unwrap();
        (
            g.t("settings_appearance_header").to_uppercase(),
            g.t("settings_theme_label").to_string(),
            g.t("settings_language_label").to_string(),
            g.t("settings_win_accent_title").to_string(),
            g.t("settings_win_accent_desc").to_string(),
            g.t("settings_custom_color_label").to_string(),
            g.live.ui_settings.theme_mode.clone(),
            g.live.ui_settings.language.clone(),
            g.live.ui_settings.use_windows_accent,
            g.live.windows_accent_hex.clone(),
            g.live.ui_settings.custom_accent_hex.clone(),
            g.language_dropdown_open,
        )
    };

    ui.label(RichText::new(hdr).size(12.0).color(pal.on_surface_muted));
    ui.add_space(8.0);

    egui::Frame::new()
        .fill(pal.surface_elevated)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // 1. Tema con iconos (Sistema, Oscuro, Claro)
            ui.horizontal(|ui| {
                ui.label(RichText::new(theme_lbl).font(semibold(14.0)).color(pal.on_surface));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    egui::Frame::new()
                        .fill(pal.surface)
                        .stroke(Stroke::new(1.0, pal.outline))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let choices = [
                                    ("system", glyph::CONTRAST, "settings_theme_system"),
                                    ("dark", glyph::DARK_MODE, "settings_theme_dark"),
                                    ("light", glyph::LIGHT_MODE, "settings_theme_light"),
                                ];
                                for (mode_k, ic, lbl_k) in choices {
                                    let lbl = { core.lock().unwrap().t(lbl_k).to_string() };
                                    let active = theme_mode == mode_k;
                                    let lbl_gal = ui.painter().layout_no_wrap(lbl.clone(), FontId::proportional(11.0), pal.on_surface);
                                    let w = (lbl_gal.size().x + 34.0).max(66.0);
                                    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
                                    if active {
                                        ui.painter().rect_filled(r, CornerRadius::same(6), pal.accent);
                                    }
                                    let fg = if active { pal.on_accent } else { pal.on_surface_muted };
                                    paint_glyph(ui.painter(), Pos2::new(r.left() + 14.0, r.center().y), ic, 14.0, fg);
                                    ui.painter().galley(
                                        Pos2::new(r.left() + 24.0, r.center().y - lbl_gal.size().y * 0.5),
                                        lbl_gal,
                                        fg,
                                    );
                                    if resp.clicked() {
                                        let mut g = core.lock().unwrap();
                                        g.live.ui_settings.theme_mode = mode_k.to_string();
                                        g.persist_ui_settings();
                                    }
                                }
                            });
                        });
                });
            });

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            // 2. Custom Language Dropdown (System first, English, Español)
            let mut lang_btn_rect = Rect::NOTHING;
            ui.horizontal(|ui| {
                ui.label(RichText::new(lang_lbl).font(semibold(14.0)).color(pal.on_surface));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let cur_lbl = {
                        let g = core.lock().unwrap();
                        let k = match lang_val.as_str() {
                            "en" => "settings_language_en",
                            "es" => "settings_language_es",
                            _ => "settings_language_system",
                        };
                        g.t(k).to_string()
                    };
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(150.0, 32.0), Sense::click());
                    lang_btn_rect = r;
                    ui.painter().rect(
                        r,
                        CornerRadius::same(8),
                        pal.surface,
                        Stroke::new(1.0, if lang_open { pal.accent } else { pal.outline }),
                        StrokeKind::Inside,
                    );
                    let cur_gal = ui.painter().layout_no_wrap(cur_lbl, FontId::proportional(12.0), pal.on_surface);
                    ui.painter().galley(
                        Pos2::new(r.left() + 12.0, r.center().y - cur_gal.size().y * 0.5),
                        cur_gal,
                        pal.on_surface,
                    );
                    paint_chevron_y(ui.painter(), Pos2::new(r.right() - 15.0, r.center().y), lang_open, 15.0, pal.on_surface_muted);
                    if resp.clicked() {
                        let mut g = core.lock().unwrap();
                        g.language_dropdown_open = !g.language_dropdown_open;
                    }
                });
            });

            if lang_open && lang_btn_rect.is_positive() {
                egui::Area::new(Id::unique("nb_lang_menu_area"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(Pos2::new(lang_btn_rect.right() - 166.0, lang_btn_rect.bottom() + 4.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::new()
                            .fill(pal.surface_card)
                            .stroke(Stroke::new(1.0, pal.outline))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::same(4))
                            .show(ui, |ui| {
                                ui.set_width(158.0);
                                let langs = [
                                    ("system", "settings_language_system"),
                                    ("en", "settings_language_en"),
                                    ("es", "settings_language_es"),
                                ];
                                for (lk, tk) in langs {
                                    let txt = { core.lock().unwrap().t(tk).to_string() };
                                    let is_sel = lang_val == lk;
                                    let (r, resp) = ui.allocate_exact_size(Vec2::new(158.0, 30.0), Sense::click());
                                    if is_sel {
                                        ui.painter().rect_filled(r, CornerRadius::same(6), pal.accent_soft);
                                    } else if resp.hovered() {
                                        ui.painter().rect_filled(r, CornerRadius::same(6), pal.surface_elevated);
                                    }
                                    let fg = if is_sel { pal.accent_on_soft } else { pal.on_surface };
                                    ui.painter().text(Pos2::new(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, txt, FontId::proportional(12.0), fg);
                                    if is_sel {
                                        paint_glyph(ui.painter(), Pos2::new(r.right() - 14.0, r.center().y), glyph::CHECK, 15.0, pal.accent);
                                    }
                                    if resp.clicked() {
                                        let mut g = core.lock().unwrap();
                                        g.live.ui_settings.language = lk.to_string();
                                        g.language_dropdown_open = false;
                                        g.persist_ui_settings();
                                    }
                                }
                            });
                    });
            }

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            // 3. Color de acento de Windows
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(win_acc_title).size(13.5).color(pal.on_surface));
                    ui.label(RichText::new(win_acc_desc).size(11.0).color(pal.on_surface_muted));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let state_lbl = { core.lock().unwrap().t(if win_acc_on { "state_on" } else { "state_off" }).to_string() };
                    if switch_pill_btn(ui, pal, &state_lbl, win_acc_on) {
                        let mut g = core.lock().unwrap();
                        g.live.ui_settings.use_windows_accent = !g.live.ui_settings.use_windows_accent;
                        g.persist_ui_settings();
                    }
                    ui.add_space(6.0);
                    let (wr, wg, wb) = parse_hex_rgb(&win_hex);
                    paint_color_hex_chip(ui, pal, Color32::from_rgb(wr, wg, wb), &win_hex);
                });
            });

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            // 4. Color personalizado (6 presets + cuentagotas con aro grueso)
            ui.horizontal(|ui| {
                ui.label(RichText::new(custom_lbl).size(12.5).color(pal.on_surface_muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let presets = ["#3B82F6", "#06B6D4", "#10B981", "#8B5CF6", "#EC4899", "#F59E0B"];
                    let cur_upper = custom_hex.to_uppercase();
                    let matched_preset = !win_acc_on && presets.iter().any(|p| *p == cur_upper);
                    let custom_active = !win_acc_on && !matched_preset;

                    // Eyedropper button
                    let (eye_r, eye_resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                    if custom_active {
                        ui.painter().circle_filled(eye_r.center(), 14.0, pal.accent);
                        ui.painter().circle_stroke(eye_r.center(), 15.5, Stroke::new(2.5, pal.accent));
                        ui.painter().circle_stroke(eye_r.center(), 13.0, Stroke::new(1.5, Color32::WHITE));
                        paint_glyph(ui.painter(), eye_r.center(), glyph::COLORIZE, 15.0, pal.on_accent);
                    } else {
                        ui.painter().circle(
                            eye_r.center(),
                            13.0,
                            pal.surface,
                            Stroke::new(1.0, if eye_resp.hovered() { pal.accent } else { pal.outline }),
                        );
                        paint_glyph(ui.painter(), eye_r.center(), glyph::COLORIZE, 15.0, pal.on_surface);
                    }
                    if eye_resp.clicked() {
                        let mut g = core.lock().unwrap();
                        let (r, gr, b) = parse_hex_rgb(&g.live.ui_settings.custom_accent_hex);
                        let (h, s, v) = rgb_to_hsv(r, gr, b);
                        g.hsv_h = if h == 0.0 && s == 0.0 { 217.0 } else { h };
                        g.hsv_s = s.max(0.05);
                        g.hsv_v = v.max(0.15);
                        g.hsv_hex_buf = rgb_to_hex(r, gr, b);
                        g.color_picker_anchor = eye_r.left_bottom();
                        g.color_picker_open = !g.color_picker_open;
                    }

                    ui.add_space(6.0);
                    for hex in presets.iter().rev() {
                        let (r, gr, b) = parse_hex_rgb(hex);
                        let c = Color32::from_rgb(r, gr, b);
                        let sel = !win_acc_on && cur_upper == *hex;
                        let (cr, cresp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                        ui.painter().circle_filled(cr.center(), 11.0, c);
                        if sel {
                            ui.painter().circle_stroke(cr.center(), 13.5, Stroke::new(2.2, c));
                        }
                        if cresp.clicked() {
                            let mut g = core.lock().unwrap();
                            g.live.ui_settings.use_windows_accent = false;
                            g.live.ui_settings.custom_accent_hex = (*hex).to_string();
                            g.persist_ui_settings();
                        }
                        ui.add_space(4.0);
                    }
                });
            });
        });
}

fn render_hsv_color_popover(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (is_open, anchor, mut h, mut s, mut v, mut hex_buf, recents, cp_title, cp_reset, cp_good, cp_low, cp_recent, cp_cancel, cp_apply) = {
        let g = core.lock().unwrap();
        if !g.color_picker_open {
            return;
        }
        (
            g.color_picker_open,
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
    let _ = is_open;

    let pos = Pos2::new((anchor.x - 280.0).clamp(16.0, 354.0), (anchor.y + 8.0).clamp(52.0, 140.0));
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
                        ui.label(RichText::new(cp_title).size(13.5).strong().color(pal.on_surface));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.link(RichText::new(cp_reset).size(11.5).color(pal.accent)).clicked() {
                                h = 217.0;
                                s = 0.76;
                                v = 0.96;
                                let (r, gr, b) = hsv_to_rgb(h, s, v);
                                hex_buf = rgb_to_hex(r, gr, b);
                            }
                        });
                    });
                    ui.add_space(10.0);

                    // SV Gradient Box
                    let (sv_rect, sv_resp) = ui.allocate_exact_size(Vec2::new(282.0, 108.0), Sense::click_and_drag());
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
                    ui.painter().rect_stroke(sv_rect, CornerRadius::same(8), Stroke::new(1.0, pal.outline), StrokeKind::Inside);

                    if (sv_resp.clicked() || sv_resp.dragged()) && sv_resp.interact_pointer_pos().is_some() {
                        let p = sv_resp.interact_pointer_pos().unwrap();
                        s = ((p.x - sv_rect.left()) / sv_rect.width()).clamp(0.0, 1.0);
                        v = 1.0 - ((p.y - sv_rect.top()) / sv_rect.height()).clamp(0.0, 1.0);
                        let (r, gr, b) = hsv_to_rgb(h, s, v);
                        hex_buf = rgb_to_hex(r, gr, b);
                    }
                    let thumb = Pos2::new(sv_rect.left() + s * sv_rect.width(), sv_rect.top() + (1.0 - v) * sv_rect.height());
                    let (cr, cg, cb) = hsv_to_rgb(h, s, v);
                    ui.painter().circle(thumb, 6.5, Color32::from_rgb(cr, cg, cb), Stroke::new(2.0, Color32::WHITE));

                    ui.add_space(10.0);

                    // Rainbow Hue Slider
                    let (hue_rect, hue_resp) = ui.allocate_exact_size(Vec2::new(282.0, 14.0), Sense::click_and_drag());
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
                    ui.painter().rect_stroke(hue_rect, CornerRadius::same(7), Stroke::new(1.0, pal.outline), StrokeKind::Inside);

                    if (hue_resp.clicked() || hue_resp.dragged()) && hue_resp.interact_pointer_pos().is_some() {
                        let p = hue_resp.interact_pointer_pos().unwrap();
                        h = (((p.x - hue_rect.left()) / hue_rect.width()).clamp(0.0, 1.0)) * 360.0;
                        let (r, gr, b) = hsv_to_rgb(h, s, v);
                        hex_buf = rgb_to_hex(r, gr, b);
                    }
                    let hthumb = Pos2::new(hue_rect.left() + (h / 360.0) * hue_rect.width(), hue_rect.center().y);
                    ui.painter().circle(hthumb, 7.5, pure_hue, Stroke::new(2.0, Color32::WHITE));

                    ui.add_space(10.0);

                    // Preview + Hex + RGB + Contrast
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
                                let (pr, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                                ui.painter().circle_filled(pr.center(), 13.0, Color32::from_rgb(cr, cg, cb));
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    let hex_resp = ui.add_sized([74.0, 20.0], egui::TextEdit::singleline(&mut hex_buf).font(FontId::monospace(11.5)));
                                    if hex_resp.changed() && hex_buf.trim().trim_start_matches('#').len() == 6 {
                                        let (nr, ng, nb) = parse_hex_rgb(&hex_buf);
                                        let (nh, ns, nv) = rgb_to_hsv(nr, ng, nb);
                                        h = nh; s = ns; v = nv;
                                    }
                                    ui.label(RichText::new(format!("RGB({}, {}, {})", cr, cg, cb)).monospace().size(10.0).color(pal.on_surface_muted));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let good = (0.28..=0.85).contains(&lum);
                                    paint_status_badge(ui, if good { &cp_good } else { &cp_low }, good, pal);
                                });
                            });
                        });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(cp_recent).size(11.0).color(pal.on_surface_muted));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            for rhex in recents.iter().rev() {
                                let (rr, rg, rb) = parse_hex_rgb(rhex);
                                let (r_rect, r_resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                                ui.painter().circle(r_rect.center(), 9.0, Color32::from_rgb(rr, rg, rb), Stroke::new(1.0, Color32::from_white_alpha(64)));
                                if r_resp.clicked() {
                                    let (nh, ns, nv) = rgb_to_hsv(rr, rg, rb);
                                    h = nh; s = ns; v = nv;
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
                            g.recent_accents.retain(|x| !x.eq_ignore_ascii_case(&final_hex));
                            g.recent_accents.insert(0, final_hex.clone());
                            g.recent_accents.truncate(5);
                            g.live.ui_settings.use_windows_accent = false;
                            g.live.ui_settings.custom_accent_hex = final_hex;
                            g.color_picker_open = false;
                            g.persist_ui_settings();
                            return;
                        }
                        ui.add_space(6.0);
                        if small_secondary_btn(ui, pal, &cp_cancel, 78.0, 28.0) {
                            core.lock().unwrap().color_picker_open = false;
                            return;
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

fn render_opt_network(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (hdr, auto_title, auto_desc, ip_lbl, ip_ph, port_lbl, reset_lbl, save_lbl, auto_on, mut ip_buf, mut port_buf, fb_opt) = {
        let g = core.lock().unwrap();
        let now = Instant::now();
        let auto_state = if g.auto_mobile_dirty_until.is_some_and(|t| now < t) {
            g.auto_mobile_optimistic
        } else {
            g.live.snapshot.auto_accept_mobile_switch
        };
        let fb = g.net_feedback.as_ref().and_then(|(s, ok, exp)| {
            if now < *exp { Some((s.clone(), *ok)) } else { None }
        });
        (
            g.t("settings_network_header").to_uppercase(),
            g.t("settings_auto_mobile_title").to_string(),
            g.t("settings_auto_mobile_desc").to_string(),
            g.t("settings_tcp_ip_label").to_string(),
            g.t("settings_tcp_ip_placeholder").to_string(),
            g.t("settings_tcp_port_label").to_string(),
            g.t("settings_reset_default").to_string(),
            g.t("settings_save_network").to_string(),
            auto_state,
            g.net_ip_buf.clone(),
            g.net_port_buf.clone(),
            fb,
        )
    };

    ui.label(RichText::new(hdr).size(12.0).color(pal.on_surface_muted));
    ui.add_space(8.0);

    egui::Frame::new()
        .fill(pal.surface_elevated)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(auto_title).font(semibold(14.0)).color(pal.on_surface));
                    ui.label(RichText::new(auto_desc).size(11.0).color(pal.on_surface_muted));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let st_lbl = { core.lock().unwrap().t(if auto_on { "state_on" } else { "state_off" }).to_string() };
                    if switch_pill_btn(ui, pal, &st_lbl, auto_on) {
                        let mut g = core.lock().unwrap();
                        g.auto_mobile_optimistic = !auto_on;
                        g.auto_mobile_dirty_until = Some(Instant::now() + Duration::from_millis(1600));
                        g.send_ipc(IpcCommand::SetAutoAcceptMobileSwitch { enabled: !auto_on });
                    }
                });
            });

            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);

            let mut ip_has_focus = false;
            let mut port_has_focus = false;
            ui.horizontal(|ui| {
                let total_w = ui.available_width();
                let ip_w = ((total_w - 8.0) * (1.4 / 2.3)).floor();
                let port_w = total_w - 8.0 - ip_w;

                ui.allocate_ui_with_layout(Vec2::new(ip_w, 52.0), Layout::top_down(Align::Min), |ui| {
                    ui.label(RichText::new(ip_lbl).size(11.0).color(pal.on_surface_muted));
                    ui.add_space(4.0);
                    let r = styled_singleline_input(ui, pal, &mut ip_buf, ip_w, 32.0, &ip_ph);
                    ip_has_focus = r.has_focus();
                });
                ui.add_space(8.0);
                ui.allocate_ui_with_layout(Vec2::new(port_w, 52.0), Layout::top_down(Align::Min), |ui| {
                    ui.label(RichText::new(port_lbl).size(11.0).color(pal.on_surface_muted));
                    ui.add_space(4.0);
                    let r = styled_singleline_input(ui, pal, &mut port_buf, port_w, 32.0, "45362");
                    port_has_focus = r.has_focus();
                    port_buf.retain(|c| c.is_ascii_digit());
                    if port_buf.len() > 5 {
                        port_buf.truncate(5);
                    }
                });
            });

            {
                let mut g = core.lock().unwrap();
                let dirty = g.net_dirty_until.is_some_and(|t| Instant::now() < t);
                if ip_has_focus || port_has_focus || dirty {
                    g.net_ip_buf = ip_buf.clone();
                    g.net_port_buf = port_buf.clone();
                } else {
                    g.net_ip_buf = g.live.snapshot.tcp_ip_override.clone().unwrap_or_default();
                    g.net_port_buf = g.live.snapshot.tcp_port.to_string();
                }
            }

            ui.add_space(10.0);
            let row_w = ui.available_width();
            ui.allocate_ui_with_layout(Vec2::new(row_w, 32.0), Layout::left_to_right(Align::Center), |ui| {
                if let Some((fb_txt, ok)) = fb_opt {
                    ui.label(RichText::new(fb_txt).size(11.0).color(if ok { pal.accent } else { pal.red_disconnect }));
                }
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 32.0), Layout::right_to_left(Align::Center), |ui| {
                    if small_accent_btn(ui, pal, &save_lbl, 96.0, 32.0) {
                        let mut g = core.lock().unwrap();
                        let parsed_port = port_buf.parse::<u16>().ok().filter(|&p| p >= 1024);
                        if let Some(p) = parsed_port {
                            let ip_opt = if ip_buf.trim().is_empty() { None } else { Some(ip_buf.trim().to_string()) };
                            g.net_ip_buf = ip_opt.clone().unwrap_or_default();
                            g.net_port_buf = p.to_string();
                            g.net_dirty_until = Some(Instant::now() + Duration::from_millis(2000));
                            g.net_feedback = Some((g.t("settings_net_saved").to_string(), true, Instant::now() + Duration::from_secs(4)));
                            g.send_ipc(IpcCommand::SetTcpNetworkConfig { tcp_ip: ip_opt, tcp_port: Some(p), reset: false });
                        } else {
                            let msg = g.t("settings_net_invalid_port").to_string();
                            g.net_feedback = Some((msg, false, Instant::now() + Duration::from_secs(4)));
                        }
                    }
                    ui.add_space(8.0);
                    let rst_gal = ui.painter().layout_no_wrap(reset_lbl.clone(), FontId::proportional(11.0), pal.on_surface_muted);
                    let rst_w = (rst_gal.size().x + 24.0).max(120.0);
                    let (rr, rresp) = ui.allocate_exact_size(Vec2::new(rst_w, 32.0), Sense::click());
                    ui.painter().rect(
                        rr,
                        CornerRadius::same(8),
                        pal.surface,
                        Stroke::new(1.0, if rresp.hovered() { pal.accent } else { pal.outline }),
                        StrokeKind::Inside,
                    );
                    ui.painter().galley(
                        Pos2::new(rr.center().x - rst_gal.size().x * 0.5, rr.center().y - rst_gal.size().y * 0.5),
                        rst_gal,
                        if rresp.hovered() { pal.on_surface } else { pal.on_surface_muted },
                    );
                    if rresp.clicked() {
                        let mut g = core.lock().unwrap();
                        g.net_ip_buf.clear();
                        g.net_port_buf = "45362".to_string();
                        g.net_dirty_until = Some(Instant::now() + Duration::from_millis(2000));
                        g.net_feedback = Some((g.t("settings_net_reset_done").to_string(), true, Instant::now() + Duration::from_secs(4)));
                        g.send_ipc(IpcCommand::SetTcpNetworkConfig { tcp_ip: None, tcp_port: None, reset: true });
                    }
                });
            });
        });
}

fn render_opt_updates(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (hdr, check_lbl, checking, spin_active, upd, sync_ver, auto_app, auto_drv, auto_on_lbl, auto_off_lbl, portable_lbl, dl_lbl, inst_drv_lbl, dl_ing_lbl, restart_lbl, app_msg, drv_msg) = {
        let g = core.lock().unwrap();
        let u = g.updates.clone();
        let is_chk = u.app_status == "checking" || u.driver_status == "checking";
        let spin_on = is_chk || g.upd_spin_until.is_some_and(|t| Instant::now() < t);
        let app_m = translate_upd_msg(&g, true, &u);
        let drv_m = translate_upd_msg(&g, false, &u);
        (
            g.t("settings_updates_header").to_uppercase(),
            g.t(if is_chk { "settings_checking_updates" } else { "settings_check_updates" }).to_string(),
            is_chk,
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
            let gal = ui.painter().layout_no_wrap(check_lbl.clone(), FontId::proportional(11.0), pal.accent);
            let w = gal.size().x + 20.0;
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::click());
            let ang = if spin_active {
                ui.ctx().request_repaint();
                (ui.input(|i| i.time) * 6.5) as f32
            } else {
                0.0
            };
            let _ = checking;
            paint_refresh_spin(ui.painter(), Pos2::new(r.left() + 7.0, r.center().y), 5.0, ang, pal.accent);
            ui.painter().galley(Pos2::new(r.left() + 18.0, r.center().y - gal.size().y * 0.5), gal, pal.accent);
            if resp.hovered() {
                ui.painter().line_segment(
                    [Pos2::new(r.left() + 18.0, r.center().y + 7.0), Pos2::new(r.right() - 2.0, r.center().y + 7.0)],
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

            // Fila App (con cápsula versión + cápsula Portable condicional)
            ui.horizontal(|ui| {
                let app_name = if upd.app_display_name.is_empty() { "NFC Bridge Sync" } else { &upd.app_display_name };
                ui.label(RichText::new(app_name).font(semibold(14.0)).color(pal.on_surface));
                ui.add_space(6.0);
                let v_str = format!("v{}", if upd.app_version.is_empty() { &sync_ver } else { &upd.app_version });
                paint_inline_capsule(ui, &v_str, true, pal);
                if upd.is_portable {
                    ui.add_space(4.0);
                    paint_inline_capsule(ui, &portable_lbl, true, pal);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if switch_pill_btn(ui, pal, if auto_app { &auto_on_lbl } else { &auto_off_lbl }, auto_app) {
                        let mut g = core.lock().unwrap();
                        g.live.ui_settings.auto_update_app = !auto_app;
                        g.persist_ui_settings();
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(RichText::new(app_msg).size(11.0).color(if upd.app_status == "available" { pal.accent } else { pal.on_surface_muted }));

            if upd.app_status == "available" || upd.app_status == "downloading" || upd.app_status == "installed_restart" || upd.pending_restart {
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

            // Fila Driver
            ui.horizontal(|ui| {
                ui.label(RichText::new("NFC Bridge Driver").font(semibold(14.0)).color(pal.on_surface));
                ui.add_space(6.0);
                let dv_str = upd.driver_version.as_deref().map(|v| format!("v{}", v)).unwrap_or_else(|| "--".to_string());
                paint_inline_capsule(ui, &dv_str, true, pal);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if switch_pill_btn(ui, pal, if auto_drv { &auto_on_lbl } else { &auto_off_lbl }, auto_drv) {
                        let mut g = core.lock().unwrap();
                        g.live.ui_settings.auto_update_driver = !auto_drv;
                        g.persist_ui_settings();
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(RichText::new(drv_msg).size(11.0).color(if upd.driver_status == "available" { pal.accent } else { pal.on_surface_muted }));

            if upd.driver_status == "available" || upd.driver_status == "downloading" {
                ui.add_space(6.0);
                let lbl = if upd.driver_status == "downloading" { &dl_ing_lbl } else { &inst_drv_lbl };
                if small_accent_btn(ui, pal, lbl, 148.0, 28.0) && upd.driver_status != "downloading" {
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
    let status = if is_app { &u.app_status } else { &u.driver_status };
    let avail = if is_app { &u.app_available } else { &u.driver_available };
    let cur = if is_app {
        Some(if u.app_version.is_empty() { g.live.sync_version.clone() } else { u.app_version.clone() })
    } else {
        u.driver_version.clone()
    };
    match status.as_str() {
        "checking" => g.t("upd_checking").to_string(),
        "up_to_date" => cur.map(|v| g.t_vars("upd_uptodate", &[("ver", &v)])).unwrap_or_else(|| g.t("upd_idle").to_string()),
        "available" => {
            if let Some(a) = avail {
                g.t_vars(if is_app { "upd_available_short" } else { "upd_driver_available" }, &[("ver", &a.version), ("asset", &a.asset_name)])
            } else {
                g.t("upd_idle").to_string()
            }
        }
        "downloading" => {
            let asset = avail.as_ref().map(|a| a.asset_name.as_str()).unwrap_or("");
            g.t_vars("upd_downloading", &[("asset", asset)])
        }
        "installed_restart" => avail.as_ref().map(|a| g.t_vars("upd_restart_ready", &[("ver", &a.version)])).unwrap_or_else(|| g.t("upd_idle").to_string()),
        "no_releases" => {
            if is_app {
                let n = if u.app_display_name.is_empty() { "NFC Bridge Sync" } else { &u.app_display_name };
                g.t_vars("upd_no_releases", &[("name", n)])
            } else {
                g.t("upd_no_releases_driver").to_string()
            }
        }
        _ => g.t("upd_idle").to_string(),
    }
}

fn render_opt_about(core: &Arc<Mutex<SharedUiCore>>, ui: &mut egui::Ui, pal: &ThemePalette) {
    let (hdr, app_name, app_ver, mode_lbl, mode_val, drv_ver_lbl, drv_state_lbl, drv_state_val, drv_online, proto_lbl, proto_val) = {
        let g = core.lock().unwrap();
        let u = &g.updates;
        let name = if u.app_display_name.is_empty() { "NFC Bridge Sync".to_string() } else { u.app_display_name.clone() };
        let ver = format!("v{} ({})", g.live.sync_version, g.live.sync_build_id);
        let dver = match (&g.live.snapshot.driver_version, &g.live.snapshot.driver_build_id) {
            (Some(v), Some(b)) => format!("v{} ({})", v, b),
            (Some(v), None) => format!("v{}", v),
            _ => g.t("about_driver_missing").to_string(),
        };
        let dst = g.t(if g.live.driver_online { "about_driver_active" } else { "about_driver_stopped" }).to_string();
        (
            g.t("settings_about_header").to_uppercase(),
            name,
            ver,
            g.t("about_exec_mode_label").to_string(),
            g.t(if u.is_portable { "about_exec_portable" } else { "about_exec_installed" }).to_string(),
            dver,
            g.t("about_driver_state_label").to_string(),
            dst,
            g.live.driver_online,
            g.t("about_protocol_label").to_string(),
            g.t("about_protocol_val").to_string(),
        )
    };

    ui.label(RichText::new(hdr).size(12.0).color(pal.on_surface_muted));
    ui.add_space(8.0);

    egui::Frame::new()
        .fill(pal.surface_elevated)
        .stroke(Stroke::new(1.0, pal.outline))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // Orden exacto HTML Image 11:
            // 1. NFC Bridge Sync
            about_row(ui, pal, &app_name, &app_ver, true, pal.on_surface);
            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);
            // 2. NFC Bridge Driver
            about_row(ui, pal, "NFC Bridge Driver", &drv_ver_lbl, true, pal.on_surface);
            ui.add_space(10.0);
            paint_divider(ui, pal);
            ui.add_space(10.0);
            // 3-5. Execution mode, Driver state (verde!), Protocol
            about_row(ui, pal, &mode_lbl, &mode_val, false, pal.on_surface_muted);
            ui.add_space(8.0);
            about_row(
                ui,
                pal,
                &drv_state_lbl,
                &drv_state_val,
                false,
                if drv_online { pal.led_green } else { pal.red_disconnect },
            );
            ui.add_space(8.0);
            about_row(ui, pal, &proto_lbl, &proto_val, false, pal.on_surface_muted);
        });
}

// ==================== 5. CUSTOM SYSTEM TRAY POPUP (STITCH tray.html) ====================

fn render_tray_viewport(core: &Arc<Mutex<SharedUiCore>>, tray_title: &str, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    // Auto-ocultar cuando pierde el foco (después de 250 ms de gracia al abrir)
    let focused = ctx.input(|i| i.viewport().focused);
    {
        let mut g = core.lock().unwrap();
        if focused == Some(false) && g.tray_opened_at.is_some_and(|t| t.elapsed() > Duration::from_millis(250)) {
            g.tray_open = false;
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            g.tray_open = false;
            return;
        }
    }

    let (pal, app_name, pc_badge, led_color, status_txt, open_lbl, switch_lbl, devices, active_id, selected_id, dev_expanded, qr_lbl, paused, pause_lbl, usb_prio, usb_lbl, opt_lbl, connected, disc_lbl, exit_lbl) = {
        let mut g = core.lock().unwrap();
        let pal = g.palette();
        let dwm_key = (pal.is_dark, pal.outline);
        if g.last_dwm_tray_key != Some(dwm_key) {
            g.last_dwm_tray_key = Some(dwm_key);
            apply_win32_dwm_to_title(tray_title, &pal);
        }
        let st = &g.live;
        let snap = &st.snapshot;
        let (l_c, s_txt) = if !st.driver_online {
            (pal.red_disconnect, g.t("device_driver_stopped").to_string())
        } else if st.paused {
            (pal.amber_pause, g.t("device_reader_paused").to_string())
        } else if st.connected {
            let tr = match st.transport.as_deref() {
                Some("usb") => "USB",
                Some("bluetooth") => "BT",
                Some("relay") => "Link",
                Some("wifi_direct") => "WFD",
                _ => "TCP",
            };
            (pal.led_green, format!("{} • {} {} ms", st.device_name.as_deref().unwrap_or("Mobile"), tr, st.latency_ms))
        } else {
            (pal.on_surface_muted, g.t("device_disconnected_short").to_string())
        };
        let app_n = if !g.updates.app_display_name.is_empty() {
            g.updates.app_display_name.clone()
        } else {
            g.t("app_sync_name").to_string()
        };
        (
            pal,
            app_n,
            g.t("tray_pc_badge").to_string(),
            l_c,
            s_txt,
            g.t("tray_open_main").to_string(),
            g.t("tray_switch_device").to_string(),
            snap.trusted_devices.clone(),
            st.active_device_id.clone(),
            st.selected_device_id.clone(),
            g.tray_devices_expanded,
            g.t("tray_show_qr").to_string(),
            st.paused,
            g.t(if st.paused { "tray_resume_reader" } else { "tray_pause_reader" }).to_string(),
            snap.usb_auto_connect,
            g.t("tray_usb_priority").to_string(),
            g.t("tray_options").to_string(),
            st.connected,
            g.t("tray_disconnect").to_string(),
            g.t("tray_exit").to_string(),
        )
    };

    egui::Frame::new()
        .fill(pal.surface_card)
        .stroke(Stroke::new(1.0, pal.outline))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);

            // Cabecera Stitch
            egui::Frame::new().inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (ic_r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                    ui.painter().rect(ic_r, CornerRadius::same(7), pal.accent_soft, Stroke::new(1.0, pal.accent_border), StrokeKind::Inside);
                    paint_glyph(ui.painter(), ic_r.center(), glyph::CONTACTLESS, 16.0, pal.accent);
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(app_name).size(13.0).strong().color(pal.on_surface));
                        ui.horizontal(|ui| {
                            let (lr, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                            ui.painter().circle_filled(lr.center(), 3.2, led_color);
                            ui.add_space(4.0);
                            ui.label(RichText::new(status_txt).size(11.0).color(pal.on_surface_muted));
                        });
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        paint_inline_capsule(ui, &pc_badge, true, &pal);
                    });
                });
            });

            paint_divider(ui, &pal);

            if tray_menu_item(ui, &pal, glyph::OPEN_IN_NEW, pal.accent, &open_lbl, pal.on_surface, true, None, false) {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.main_visible = true;
                g.main_want_focus = true;
            }

            // Selector rápido de dispositivo móvil
            let chev_ic = if dev_expanded { glyph::EXPAND_MORE } else { glyph::CHEVRON_RIGHT };
            if tray_menu_item(ui, &pal, glyph::SMARTPHONE, pal.on_surface_muted, &switch_lbl, pal.on_surface, false, Some((chev_ic, pal.on_surface_muted)), false) {
                let mut g = core.lock().unwrap();
                g.tray_devices_expanded = !g.tray_devices_expanded;
            }
            if dev_expanded {
                for dev in devices.iter().take(4) {
                    let is_act = Some(&dev.device_id) == active_id.as_ref() || Some(&dev.device_id) == selected_id.as_ref();
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
                    let inner = Rect::from_min_max(Pos2::new(r.left() + 26.0, r.top()), r.max);
                    if is_act {
                        ui.painter().rect_filled(inner, CornerRadius::same(6), pal.surface_elevated);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(inner, CornerRadius::same(6), pal.surface_elevated);
                    }
                    ui.painter().text(
                        Pos2::new(inner.left() + 8.0, inner.center().y),
                        egui::Align2::LEFT_CENTER,
                        &dev.device_name,
                        FontId::proportional(11.5),
                        if is_act { pal.on_surface } else { pal.on_surface_muted },
                    );
                    if is_act {
                        paint_glyph(ui.painter(), Pos2::new(inner.right() - 12.0, inner.center().y), glyph::CHECK, 14.0, pal.led_green);
                    }
                    if resp.clicked() {
                        let id = dev.device_id.clone();
                        let g = core.lock().unwrap();
                        let ipc = g.ipc.clone();
                        g.rt.spawn(async move {
                            ipc.select_saved_profile(&id).await;
                            let _ = ipc.send_command(IpcCommand::SelectTrustedDevice { device_id: id }).await;
                        });
                    }
                }
            }

            if tray_menu_item(ui, &pal, glyph::QR_CODE, pal.on_surface_muted, &qr_lbl, pal.on_surface, false, None, false) {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.main_visible = true;
                g.main_want_focus = true;
                g.active_panel = "tcp".to_string();
                drop(g);
                trigger_qr_request(core, &ctx, "tcp");
            }

            paint_divider(ui, &pal);

            let p_ic = if paused { glyph::PLAY_ARROW } else { glyph::PAUSE };
            let p_ic_col = if paused { pal.amber_pause } else { pal.on_surface_muted };
            if tray_menu_item(ui, &pal, p_ic, p_ic_col, &pause_lbl, pal.on_surface, false, None, false) {
                core.lock().unwrap().send_ipc(IpcCommand::SetPaused { paused: !paused });
            }

            let usb_right = if usb_prio { Some((glyph::CHECK, pal.accent)) } else { None };
            if tray_menu_item(ui, &pal, glyph::USB, pal.on_surface_muted, &usb_lbl, pal.on_surface, false, usb_right, false) {
                core.lock().unwrap().send_ipc(IpcCommand::SetUsbPriority { enabled: !usb_prio });
            }

            paint_divider(ui, &pal);

            if tray_menu_item(ui, &pal, glyph::SETTINGS, pal.on_surface_muted, &opt_lbl, pal.on_surface, false, None, false) {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.options_open = true;
                g.options_want_focus = true;
            }

            let disc_col = if connected { pal.red_disconnect } else { pal.red_disconnect.gamma_multiply(0.4) };
            if tray_menu_item(ui, &pal, glyph::LINK_OFF, disc_col, &disc_lbl, disc_col, false, None, true) && connected {
                core.lock().unwrap().send_ipc(IpcCommand::DisconnectDevice);
            }

            if tray_menu_item(ui, &pal, glyph::POWER_SETTINGS_NEW, pal.red_disconnect, &exit_lbl, pal.red_disconnect, false, None, true) {
                let mut g = core.lock().unwrap();
                g.tray_open = false;
                g.should_quit = true;
            }
        });
}

// ==================== 6. UI WIDGET HELPERS ====================

fn paint_pill(ui: &egui::Ui, center_left: Pos2, text: &str, font_size: f32, mono: bool, translucent: bool, pal: &ThemePalette) -> f32 {
    let font = if mono { FontId::monospace(font_size) } else { FontId::proportional(font_size) };
    let gal = ui.painter().layout_no_wrap(text.to_string(), font, pal.on_surface_muted);
    let w = gal.size().x + 16.0;
    let rect = Rect::from_min_size(Pos2::new(center_left.x, center_left.y - 10.5), Vec2::new(w, 21.0));
    let bg = if translucent { pal.build_badge_bg } else { pal.surface_elevated };
    ui.painter().rect(rect, CornerRadius::same(11), bg, Stroke::new(1.0, pal.outline), StrokeKind::Inside);
    ui.painter().galley(Pos2::new(rect.left() + 8.0, rect.center().y - gal.size().y * 0.5), gal, pal.on_surface_muted);
    rect.right()
}

fn paint_inline_capsule(ui: &mut egui::Ui, text: &str, mono: bool, pal: &ThemePalette) {
    let font = if mono { FontId::monospace(10.0) } else { FontId::proportional(10.0) };
    let gal = ui.painter().layout_no_wrap(text.to_string(), font, pal.on_surface_muted);
    let (r, _) = ui.allocate_exact_size(Vec2::new(gal.size().x + 14.0, 19.0), Sense::hover());
    ui.painter().rect(r, CornerRadius::same(10), pal.surface, Stroke::new(1.0, pal.outline), StrokeKind::Inside);
    ui.painter().galley(Pos2::new(r.left() + 7.0, r.center().y - gal.size().y * 0.5), gal, pal.on_surface_muted);
}

fn paint_chrome_btn(painter: &egui::Painter, rect: Rect, resp: &egui::Response, ic: char, danger: bool, pal: &ThemePalette) {
    if resp.hovered() {
        let bg = if danger { pal.red_soft } else { pal.surface_elevated };
        painter.rect_filled(rect, CornerRadius::same(8), bg);
    }
    let fg = if resp.hovered() && danger { pal.red_disconnect } else if resp.hovered() { pal.on_surface } else { pal.on_surface_muted };
    paint_glyph(painter, rect.center(), ic, 18.0, fg);
}

fn paint_status_badge(ui: &mut egui::Ui, text: &str, active: bool, pal: &ThemePalette) {
    let fg = if active { pal.accent_on_soft } else { pal.on_surface_muted };
    let bg = if active { pal.accent_soft } else { pal.surface_elevated };
    let stroke = if active { pal.accent_border } else { pal.outline };
    let gal = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(11.0), fg);
    let (r, _) = ui.allocate_exact_size(Vec2::new(gal.size().x + 16.0, 21.0), Sense::hover());
    ui.painter().rect(r, CornerRadius::same(11), bg, Stroke::new(1.0, stroke), StrokeKind::Inside);
    ui.painter().galley(Pos2::new(r.left() + 8.0, r.center().y - gal.size().y * 0.5), gal, fg);
}

fn paint_divider(ui: &mut egui::Ui, pal: &ThemePalette) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, pal.outline));
}

fn toggle_state_btn(ui: &mut egui::Ui, pal: &ThemePalette, ic: char, label: &str, state_txt: &str, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        pal.surface_elevated,
        Stroke::new(1.0, if resp.hovered() { pal.accent } else { pal.outline }),
        StrokeKind::Inside,
    );
    paint_glyph(ui.painter(), Pos2::new(r.left() + 20.0, r.center().y), ic, 16.0, if on { pal.accent } else { pal.on_surface_muted });
    ui.painter().text(Pos2::new(r.left() + 34.0, r.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(12.0), pal.on_surface);
    ui.painter().text(
        Pos2::new(r.right() - 12.0, r.center().y),
        egui::Align2::RIGHT_CENTER,
        state_txt,
        FontId::monospace(10.0),
        if on { pal.led_green } else { pal.on_surface_muted },
    );
    resp.clicked()
}

fn info_copy_tile_w(ui: &mut egui::Ui, pal: &ThemePalette, w: f32, label: &str, value: &str, copied: bool) -> bool {
    info_copy_tile_w_col(ui, pal, w, label, value, copied, pal.on_surface)
}

fn info_copy_tile_w_col(ui: &mut egui::Ui, pal: &ThemePalette, w: f32, label: &str, value: &str, copied: bool, val_col: Color32) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 50.0), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        pal.surface_elevated,
        Stroke::new(1.0, if resp.hovered() { pal.accent_border } else { pal.outline }),
        StrokeKind::Inside,
    );
    ui.painter().text(Pos2::new(r.left() + 12.0, r.top() + 8.0), egui::Align2::LEFT_TOP, label, FontId::proportional(11.0), pal.on_surface_muted);
    ui.painter().text(Pos2::new(r.left() + 12.0, r.top() + 25.0), egui::Align2::LEFT_TOP, value, FontId::monospace(12.0), val_col);
    paint_glyph(ui.painter(), Pos2::new(r.right() - 16.0, r.top() + 32.0), if copied { glyph::CHECK } else { glyph::CONTENT_COPY }, 14.0, if copied { pal.led_green } else if resp.hovered() { pal.accent } else { pal.on_surface_muted });
    resp.clicked()
}

fn info_kv_row(ui: &mut egui::Ui, pal: &ThemePalette, label: &str, value: &str, elevated_bg: bool) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        if elevated_bg { pal.surface_elevated } else { pal.surface },
        Stroke::new(1.0, pal.outline),
        StrokeKind::Inside,
    );
    ui.painter().text(Pos2::new(r.left() + 12.0, r.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(12.0), pal.on_surface_muted);
    ui.painter().text(Pos2::new(r.right() - 12.0, r.center().y), egui::Align2::RIGHT_CENTER, value, FontId::monospace(12.0), pal.on_surface);
}

fn primary_cta_btn(ui: &mut egui::Ui, pal: &ThemePalette, ic: char, label: &str, active_style: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
    let bg = if active_style {
        if resp.hovered() { pal.accent.gamma_multiply(0.92) } else { pal.accent }
    } else if resp.hovered() {
        pal.surface_elevated
    } else {
        pal.surface_elevated
    };
    let fg = if active_style { pal.on_accent } else { pal.on_surface_muted };
    ui.painter().rect(r, CornerRadius::same(8), bg, if active_style { Stroke::NONE } else { Stroke::new(1.0, if resp.hovered() { pal.accent } else { pal.outline }) }, StrokeKind::Inside);

    let gal = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(14.0), fg);
    let total_w = 22.0 + gal.size().x;
    let x0 = r.center().x - total_w * 0.5;
    paint_glyph(ui.painter(), Pos2::new(x0 + 8.0, r.center().y), ic, 16.0, fg);
    ui.painter().galley(Pos2::new(x0 + 22.0, r.center().y - gal.size().y * 0.5), gal, fg);
    resp.clicked()
}

fn secondary_btn(ui: &mut egui::Ui, pal: &ThemePalette, ic: char, label: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let bg = if resp.hovered() { pal.outline } else { pal.surface_elevated };
    ui.painter().rect(r, CornerRadius::same(8), bg, Stroke::new(1.0, pal.outline), StrokeKind::Inside);
    let gal = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.0), pal.on_surface);
    let total_w = 22.0 + gal.size().x;
    let x0 = r.center().x - total_w * 0.5;
    paint_glyph(ui.painter(), Pos2::new(x0 + 8.0, r.center().y), ic, 16.0, pal.on_surface_muted);
    ui.painter().galley(Pos2::new(x0 + 22.0, r.center().y - gal.size().y * 0.5), gal, pal.on_surface);
    resp.clicked()
}

fn secondary_btn_accent_icon(ui: &mut egui::Ui, pal: &ThemePalette, ic: char, label: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    ui.painter().rect(r, CornerRadius::same(8), pal.surface_elevated, Stroke::new(1.0, if resp.hovered() { pal.accent } else { pal.outline }), StrokeKind::Inside);
    let gal = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.0), pal.on_surface);
    let total_w = 22.0 + gal.size().x;
    let x0 = r.center().x - total_w * 0.5;
    paint_glyph(ui.painter(), Pos2::new(x0 + 8.0, r.center().y), ic, 16.0, pal.accent);
    ui.painter().galley(Pos2::new(x0 + 22.0, r.center().y - gal.size().y * 0.5), gal, pal.on_surface);
    resp.clicked()
}

fn small_ghost_btn(ui: &mut egui::Ui, pal: &ThemePalette, ic: char, label: &str) -> bool {
    let gal = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.0), pal.on_surface);
    let w = gal.size().x + 32.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(6), pal.surface_elevated);
    }
    paint_glyph(ui.painter(), Pos2::new(r.left() + 12.0, r.center().y), ic, 15.0, pal.on_surface_muted);
    ui.painter().galley(Pos2::new(r.left() + 24.0, r.center().y - gal.size().y * 0.5), gal, pal.on_surface);
    resp.clicked()
}

fn small_accent_btn(ui: &mut egui::Ui, pal: &ThemePalette, label: &str, w: f32, h: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    ui.painter().rect_filled(r, CornerRadius::same(8), if resp.hovered() { pal.accent.gamma_multiply(0.92) } else { pal.accent });
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.0), pal.on_accent);
    resp.clicked()
}

fn small_secondary_btn(ui: &mut egui::Ui, pal: &ThemePalette, label: &str, w: f32, h: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    ui.painter().rect(r, CornerRadius::same(8), if resp.hovered() { pal.surface } else { pal.surface_elevated }, Stroke::new(1.0, pal.outline), StrokeKind::Inside);
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.0), pal.on_surface);
    resp.clicked()
}

fn small_danger_btn(ui: &mut egui::Ui, pal: &ThemePalette, label: &str, w: f32, h: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    ui.painter().rect(r, CornerRadius::same(8), pal.red_soft, Stroke::new(1.0, pal.red_border), StrokeKind::Inside);
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.0), pal.red_disconnect);
    resp.clicked()
}

fn styled_singleline_input(
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

fn truncate_text_to_width(painter: &egui::Painter, text: &str, font: FontId, max_w: f32) -> String {
    if painter.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE).size().x <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let cand: String = chars[..mid].iter().collect::<String>() + "...";
        if painter.layout_no_wrap(cand, font.clone(), Color32::WHITE).size().x <= max_w {
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

fn switch_pill_btn(ui: &mut egui::Ui, pal: &ThemePalette, label: &str, on: bool) -> bool {
    let fg = if on { pal.accent_on_soft } else { pal.on_surface_muted };
    let gal = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(11.0), fg);
    let w = (gal.size().x + 20.0).max(74.0);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    ui.painter().rect(
        r,
        CornerRadius::same(8),
        if on { pal.accent_soft } else { pal.surface },
        Stroke::new(1.0, if on { pal.accent_border } else { pal.outline }),
        StrokeKind::Inside,
    );
    ui.painter().galley(Pos2::new(r.center().x - gal.size().x * 0.5, r.center().y - gal.size().y * 0.5), gal, fg);
    resp.clicked()
}

fn paint_color_hex_chip(ui: &mut egui::Ui, pal: &ThemePalette, dot_col: Color32, hex: &str) {
    let gal = ui.painter().layout_no_wrap(hex.to_string(), FontId::monospace(10.0), pal.on_surface_muted);
    let (r, _) = ui.allocate_exact_size(Vec2::new(gal.size().x + 26.0, 22.0), Sense::hover());
    ui.painter().rect(r, CornerRadius::same(11), pal.surface, Stroke::new(1.0, pal.outline), StrokeKind::Inside);
    ui.painter().circle_filled(Pos2::new(r.left() + 11.0, r.center().y), 4.5, dot_col);
    ui.painter().galley(Pos2::new(r.left() + 20.0, r.center().y - gal.size().y * 0.5), gal, pal.on_surface_muted);
}

fn about_row(ui: &mut egui::Ui, pal: &ThemePalette, left: &str, right: &str, left_strong: bool, right_col: Color32) {
    ui.horizontal(|ui| {
        if left_strong {
            ui.label(RichText::new(left).font(semibold(14.0)).color(pal.on_surface));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(right).monospace().size(12.0).color(right_col));
            });
        } else {
            ui.label(RichText::new(left).size(11.0).color(pal.on_surface_muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(right).monospace().size(11.0).color(right_col));
            });
        }
    });
}

fn tray_menu_item(
    ui: &mut egui::Ui,
    pal: &ThemePalette,
    ic: char,
    ic_col: Color32,
    label: &str,
    lbl_col: Color32,
    strong: bool,
    right_ic: Option<(char, Color32)>,
    danger_hover: bool,
) -> bool {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(7), if danger_hover { pal.red_soft } else { pal.surface_elevated });
    }
    paint_glyph(ui.painter(), Pos2::new(r.left() + 18.0, r.center().y), ic, 16.0, ic_col);
    let mut font = FontId::proportional(12.5);
    if strong {
        font = FontId::proportional(13.0);
    }
    ui.painter().text(Pos2::new(r.left() + 36.0, r.center().y), egui::Align2::LEFT_CENTER, label, font, lbl_col);
    if let Some((ric, rcol)) = right_ic {
        paint_glyph(ui.painter(), Pos2::new(r.right() - 14.0, r.center().y), ric, 16.0, rcol);
    }
    resp.clicked()
}

fn build_nfc_rgba_bytes() -> Vec<u8> {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - 15.5).abs();
            let dy = (y as f32 - 15.5).abs();
            let idx = (y * size + x) * 4;
            let corner = if dx > 10.5 && dy > 10.5 {
                ((dx - 10.5).powi(2) + (dy - 10.5).powi(2)).sqrt() <= 5.0
            } else {
                dx <= 15.5 && dy <= 15.5
            };
            if corner {
                let cx = x as f32 - 11.5;
                let cy = y as f32 - 15.5;
                let dist = (cx * cx + cy * cy).sqrt();
                let in_arc = cx >= -1.0
                    && ((dist >= 3.2 && dist <= 5.4)
                        || (dist >= 7.6 && dist <= 9.8)
                        || (dist >= 12.0 && dist <= 14.2));
                let in_dot = ((x as f32 - 10.5).powi(2) + (y as f32 - 15.5).powi(2)).sqrt() <= 2.1;
                if in_arc || in_dot {
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                    rgba[idx + 3] = 255;
                } else {
                    rgba[idx] = 59;
                    rgba[idx + 1] = 130;
                    rgba[idx + 2] = 246;
                    rgba[idx + 3] = 255;
                }
            }
        }
    }
    rgba
}
