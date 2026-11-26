//! Interfaz de escritorio 100 % Rust nativa (`egui` + `eframe`) para NFC Bridge Sync y Tools Desktop.
//!
//! Módulos organizados por responsabilidad:
//! - `main_window`: ventana principal, barra superior, tarjetas de dispositivos y paneles de transporte/QR
//! - `options`: ventana independiente de opciones (`Apariencia`, `Conexión y red`, `Actualizaciones`, `Acerca de`, selector HSV)
//! - `tray`: menú emergente personalizado en bandeja del sistema
//! - `widgets`, `theme`, `icons`, `qr`, `state`: primitivas visuales, sistema de diseño y estado UI

#![cfg(target_os = "windows")]

mod icons;
mod main_window;
mod options;
mod qr;
mod state;
mod theme;
mod tray;
mod widgets;

use crate::i18n::I18nCatalog;
use crate::ipc::{DriverIpcClient, IpcCommand};
#[cfg(windows)]
use crate::platform::{
    configure_tray_popup_win32, current_tray_input_mask_win32, hide_tray_popup_win32,
    restore_main_window_win32, show_tray_popup_at_win32,
};
use crate::updater::DesktopUpdater;
use eframe::egui;
use egui::{Pos2, ViewportBuilder, ViewportCommand, ViewportId};
use icons::setup_fonts;
use main_window::render_main_viewport;
use options::render_options_viewport;
use state::{OptionsTab, SharedUiCore};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use theme::{apply_egui_visuals, apply_win32_dwm_to_title, parse_hex_rgb, rgb_to_hex, rgb_to_hsv, ThemePalette};
use tray::icon::{build_nfc_rgba_bytes, load_app_icon_data};
use tray::{estimate_tray_height, render_tray_viewport};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub fn run_desktop_ui(
    app_title: &str,
    app_ico_bytes: &[u8],
    ipc: DriverIpcClient,
    updater: DesktopUpdater,
    rt_handle: tokio::runtime::Handle,
    _data_dir: std::path::PathBuf,
) -> anyhow::Result<()> {
    let eframe_icon = load_app_icon_data(app_ico_bytes);
    let tray_handle = tray_icon::Icon::from_resource(2, None)
        .ok()
        .or_else(|| tray_icon::Icon::from_rgba(build_nfc_rgba_bytes(), 32, 32).ok())
        .and_then(|ic| {
            TrayIconBuilder::new()
                .with_tooltip(app_title)
                .with_icon(ic)
                .build()
                .ok()
        });

    let init_live = rt_handle.block_on(async { ipc.get_live_state().await });
    let init_updates = rt_handle.block_on(async { updater.snapshot().await });
    let init_pal = ThemePalette::from_settings(
        &init_live.ui_settings,
        &init_live.windows_accent_hex,
    );

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
        options_just_opened: false,
        options_want_focus: false,
        options_painted: false,
        options_target_pos: None,
        tray_open: false,
        tray_input_mask: 0,
        tray_opened_at: None,
        tray_closed_at: None,
        tray_pos: None,
        tray_anchor_px: None,
        tray_devices_expanded: false,
        tray_measured_height: None,
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
        snapshot_dirty_until: None,
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

    let mut native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title(app_title)
            .with_inner_size([860.0, 504.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(false)
            .with_icon(Arc::new(eframe_icon)),
        ..Default::default()
    };
    native_options.glow_options.vsync = false;

    let tray_events: Arc<Mutex<Vec<TrayIconEvent>>> = Arc::new(Mutex::new(Vec::new()));

    let title_owned = app_title.to_string();
    let title_for_ev = title_owned.clone();
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
            let core_tray_ev = core.clone();
            let main_t_ev = title_for_ev.clone();
            let tray_t_ev = format!("{} - Tray", title_for_ev);
            TrayIconEvent::set_event_handler(Some(move |ev| {
                if let TrayIconEvent::Click {
                    button,
                    button_state: MouseButtonState::Up,
                    position,
                    ..
                } = ev
                {
                    match button {
                        MouseButton::Left => {
                            {
                                let mut g = core_tray_ev.lock().unwrap();
                                g.tray_open = false;
                                g.tray_closed_at = Some(Instant::now());
                                g.main_visible = true;
                                g.main_want_focus = true;
                            }
                            #[cfg(windows)]
                            {
                                hide_tray_popup_win32(&tray_t_ev);
                                restore_main_window_win32(&main_t_ev);
                            }
                            ctx_tray.request_repaint_of(ViewportId::ROOT);
                        }
                        MouseButton::Right => {
                            let ppp = ctx_tray.pixels_per_point().max(1.0);
                            let mut opened = false;
                            let mut closed = false;
                            {
                                let mut g = core_tray_ev.lock().unwrap();
                                if g.tray_open {
                                    g.tray_open = false;
                                    g.tray_closed_at = Some(Instant::now());
                                    closed = true;
                                } else if g
                                    .tray_closed_at
                                    .is_none_or(|t| t.elapsed() > Duration::from_millis(160))
                                {
                                    let est_h = estimate_tray_height(&g);
                                    let ax = position.x.round() as i32;
                                    let ay = position.y.round() as i32;
                                    #[cfg(windows)]
                                    let clamped = show_tray_popup_at_win32(
                                        &tray_t_ev, ax, ay, 296.0, est_h, ppp,
                                    );
                                    #[cfg(not(windows))]
                                    let clamped: Option<(f32, f32)> = None;
                                    g.tray_anchor_px = Some((ax, ay));
                                    g.tray_pos = Some(match clamped {
                                        Some((lx, ly)) => Pos2::new(lx, ly),
                                        None => Pos2::new(
                                            (position.x as f32 / ppp - 278.0).max(12.0),
                                            (position.y as f32 / ppp - est_h - 14.0).max(12.0),
                                        ),
                                    });
                                    g.tray_open = true;
                                    #[cfg(windows)]
                                    {
                                        g.tray_input_mask = current_tray_input_mask_win32();
                                    }
                                    g.tray_opened_at = Some(Instant::now());
                                    opened = true;
                                }
                            }
                            if closed {
                                #[cfg(windows)]
                                hide_tray_popup_win32(&tray_t_ev);
                                ctx_tray.request_repaint_of(ViewportId::ROOT);
                            } else if opened {
                                ctx_tray.request_repaint_of(ViewportId::from_hash_of(
                                    "nb_tray_popup_viewport",
                                ));
                                ctx_tray.request_repaint_of(ViewportId::ROOT);
                            }
                        }
                        _ => {}
                    }
                }
            }));

            spawn_state_sync_loop(
                &rt_handle,
                cc.egui_ctx.clone(),
                core.clone(),
                ipc,
                updater,
            );

            Ok(Box::new(NfcBridgeEguiApp {
                core,
                _tray: tray_handle,
                tray_events,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}

fn spawn_state_sync_loop(
    rt_handle: &tokio::runtime::Handle,
    ctx_bg: egui::Context,
    core_bg: Arc<Mutex<SharedUiCore>>,
    ipc_bg: DriverIpcClient,
    upd_bg: DesktopUpdater,
) {
    let notify = ipc_bg.notifier();
    rt_handle.spawn(async move {
        let mut last_live = None;
        let mut last_upd = None;
        loop {
            tokio::select! {
                _ = notify.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(1200)) => {
                    let _ = ipc_bg.send_command(IpcCommand::GetTransportSnapshot).await;
                }
            }
            let st = ipc_bg.get_live_state().await;
            upd_bg
                .set_known_driver_version(st.snapshot.driver_version.clone())
                .await;
            let upd = upd_bg.snapshot().await;

            // Comparación estructural directa sin serialización JSON (cero asignaciones en reposo)
            let changed = last_live.as_ref() != Some(&st) || last_upd.as_ref() != Some(&upd);
            if changed {
                last_live = Some(st.clone());
                last_upd = Some(upd.clone());
            }

            let (opt_open, tray_open) = {
                let mut guard = core_bg.lock().unwrap();
                let now = Instant::now();
                let keep_ui_settings = guard
                    .ui_settings_dirty_until
                    .is_some_and(|t| now < t);
                let keep_snapshot = guard.snapshot_dirty_until.is_some_and(|t| now < t);
                let saved_ui = if keep_ui_settings {
                    Some(guard.live.ui_settings.clone())
                } else {
                    None
                };
                let saved_toggles = if keep_snapshot {
                    Some((
                        guard.live.paused,
                        guard.live.snapshot.usb_enabled,
                        guard.live.snapshot.usb_auto_connect,
                        guard.live.snapshot.tcp_enabled,
                        guard.live.snapshot.bt_enabled,
                        guard.live.snapshot.wfd_enabled,
                        guard.live.snapshot.auto_accept_mobile_switch,
                    ))
                } else {
                    None
                };
                guard.live = st;
                if let Some(prev) = saved_ui {
                    guard.live.ui_settings = prev;
                } else {
                    guard.sidebar_compact = guard.live.ui_settings.settings_sidebar_compact;
                }
                if let Some((p, ue, ua, te, be, we, am)) = saved_toggles {
                    guard.live.paused = p;
                    guard.live.snapshot.usb_enabled = ue;
                    guard.live.snapshot.usb_auto_connect = ua;
                    guard.live.snapshot.tcp_enabled = te;
                    guard.live.snapshot.bt_enabled = be;
                    guard.live.snapshot.wfd_enabled = we;
                    guard.live.snapshot.auto_accept_mobile_switch = am;
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
                (guard.options_open, guard.tray_open)
            };
            if changed {
                repaint_open_viewports(&ctx_bg, opt_open, tray_open);
            }
        }
    });
}

pub(crate) fn repaint_open_viewports(ctx: &egui::Context, options_open: bool, tray_open: bool) {
    ctx.request_repaint_of(ViewportId::ROOT);
    if options_open {
        ctx.request_repaint_of(ViewportId::from_hash_of("nb_options_viewport"));
    }
    if tray_open {
        ctx.request_repaint_of(ViewportId::from_hash_of("nb_tray_popup_viewport"));
    }
}

pub(crate) fn repaint_active_from_guard(ctx: &egui::Context, g: &SharedUiCore) {
    repaint_open_viewports(ctx, g.options_open, g.tray_open);
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
        let _ = &self.tray_events;

        if ctx.input(|i| i.viewport().close_requested()) {
            let mut g = self.core.lock().unwrap();
            if !g.should_quit {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                g.main_visible = false;
            }
        }

        let (
            want_focus_main,
            main_visible,
            main_title,
            pal,
            options_open,
            options_just_opened,
            options_want_focus,
            options_painted,
            mut options_target_pos,
            tray_open,
            tray_pos,
            tray_h,
            should_quit,
            needs_dwm_main,
            needs_visuals,
        ) = {
            let mut g = self.core.lock().unwrap();
            if g.should_quit {
                g.tray_open = false;
                g.options_open = false;
            }
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

            let ojo = g.options_just_opened;
            g.options_just_opened = false;

            let owf = g.options_want_focus;
            g.options_want_focus = false;

            let th = estimate_tray_height(&g);
            (
                wf,
                g.main_visible,
                g.app_title.clone(),
                pal,
                g.options_open,
                ojo,
                owf,
                g.options_painted,
                g.options_target_pos,
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
            ctx.request_repaint_of(ViewportId::ROOT);
            return;
        }

        if want_focus_main {
            #[cfg(windows)]
            restore_main_window_win32(&main_title);
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
                g.options_painted = false;
                g.options_target_pos = None;
            }
            if !tray_open {
                g.last_dwm_tray_key = None;
            }
        }

        if options_open {
            let opt_id = ViewportId::from_hash_of("nb_options_viewport");
            let opt_title = format!("{} - Options", main_title);
            if options_just_opened || options_target_pos.is_none() {
                let tp = if main_visible {
                    if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                        if outer.min.x > -10000.0 && outer.min.y > -10000.0 && outer.width() > 200.0 {
                            Pos2::new(
                                (outer.center().x - 340.0).max(20.0),
                                (outer.center().y - 234.0).max(20.0),
                            )
                        } else {
                            Pos2::new(240.0, 140.0)
                        }
                    } else {
                        Pos2::new(240.0, 140.0)
                    }
                } else if let Some(mon) = ctx.input(|i| i.viewport().monitor_size) {
                    Pos2::new(
                        ((mon.x - 680.0) * 0.5).max(20.0),
                        ((mon.y - 468.0) * 0.5).max(20.0),
                    )
                } else {
                    Pos2::new(240.0, 140.0)
                };
                self.core.lock().unwrap().options_target_pos = Some(tp);
                options_target_pos = Some(tp);
            }
            let mut vb = ViewportBuilder::default()
                .with_title(opt_title.clone())
                .with_inner_size([680.0, 468.0])
                .with_decorations(false)
                .with_resizable(false)
                .with_transparent(false);
            if !options_painted {
                // Crear el HWND fuera de pantalla hasta que complete su primer swap_buffers
                // para evitar el destello negro inicial al abrir Opciones.
                vb = vb.with_position(Pos2::new(-32000.0, -32000.0));
            } else if let Some(tp) = options_target_pos {
                vb = vb.with_position(tp);
            }
            let core_opt = self.core.clone();
            ctx.show_viewport_deferred(opt_id, vb, move |ui, _class| {
                render_options_viewport(&core_opt, &opt_title, ui);
            });
            if !options_painted {
                ctx.request_repaint_of(opt_id);
            } else if options_want_focus {
                ctx.send_viewport_cmd_to(opt_id, ViewportCommand::Visible(true));
                ctx.send_viewport_cmd_to(opt_id, ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd_to(opt_id, ViewportCommand::Focus);
                ctx.request_repaint_of(opt_id);
            }
        }

        // Mantener el viewport del tray pre-creado en memoria para apertura instantánea (<5 ms)
        // incluso con la ventana principal minimizada o cerrada en segundo plano.
        let tray_id = ViewportId::from_hash_of("nb_tray_popup_viewport");
        let tray_title = format!("{} - Tray", main_title);
        let w = 296.0_f32;
        let pos = if tray_open {
            tray_pos.unwrap_or(Pos2::new(900.0, 420.0))
        } else {
            Pos2::new(-32000.0, -32000.0)
        };
        let vb = ViewportBuilder::default()
            .with_title(tray_title.clone())
            .with_inner_size([w, tray_h])
            .with_position(pos)
            .with_visible(tray_open)
            .with_active(false)
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(false)
            .with_always_on_top()
            .with_taskbar(false);
        let core_tray = self.core.clone();
        let tray_title_cb = tray_title.clone();
        ctx.show_viewport_deferred(tray_id, vb, move |ui, _class| {
            render_tray_viewport(&core_tray, &tray_title_cb, ui);
        });
        #[cfg(windows)]
        configure_tray_popup_win32(&tray_title);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        render_main_viewport(&self.core, &ctx, ui);
    }
}
