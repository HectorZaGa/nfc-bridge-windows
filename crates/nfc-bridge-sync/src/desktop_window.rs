//! Host de escritorio nativo sin bordes (WebView2 vía `tao` + `wry` con protocolo interno
//! `nfcbridge://localhost` — sin abrir puertos HTTP locales) para NFC Bridge Sync y Tools Desktop.
//!
//! - Ventana principal sin barra de título nativa de Windows (`decorations: false`), esquinas DWM
//!   redondeadas y botón de cerrar (`X`) que oculta la ventana en la bandeja del sistema.
//! - Ventana de Opciones independiente (`/?view=settings`) sin difuminar la ventana principal y con
//!   cierre instantáneo (se oculta para reabrir sin latencia).
//! - Menú emergente personalizado en la bandeja del sistema (`/?view=tray`) basado en el diseño Stitch.

#[cfg(target_os = "windows")]
pub fn run_desktop_ui(
    app_title: &str,
    ipc: crate::server::DriverIpcClient,
    updater: crate::updater::DesktopUpdater,
    rt_handle: tokio::runtime::Handle,
    data_dir: std::path::PathBuf,
) -> anyhow::Result<()> {
    use std::borrow::Cow;
    use std::cell::RefCell;
    use std::rc::Rc;
    use tao::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
    use tao::platform::windows::WindowBuilderExtWindows;
    use tao::window::{Icon, Window, WindowBuilder};
    use tracing::warn;
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use wry::{WebContext, WebView, WebViewBuilder};

    #[derive(Debug, Clone)]
    enum UiWindowEvent {
        DragMain,
        MinimizeMain,
        CloseMain,
        ShowMain,
        ShowMainQr,
        ReadyMain,
        ResizeMain(f64, f64),
        OpenSettings,
        DragSettings,
        MinimizeSettings,
        CloseSettings,
        ReadySettings,
        ResizeSettings(f64, f64),
        ToggleTrayPopup(PhysicalPosition<f64>),
        ReadyTray,
        CloseTray,
        ResizeTray(f64, f64),
        UiSettingsChanged,
        ThemeChanged(bool),
        QuitApp,
    }

    fn build_nfc_rgba_icon() -> Option<(Vec<u8>, u32, u32)> {
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
        Some((rgba, size as u32, size as u32))
    }

    fn apply_native_frameless_style(win: &Window, dark: bool) {
        use tao::platform::windows::WindowExtWindows;
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        };
        use windows_sys::Win32::Graphics::Gdi::CreateSolidBrush;
        use windows_sys::Win32::UI::WindowsAndMessaging::{SetClassLongPtrW, GCLP_HBRBACKGROUND};

        let hwnd = win.hwnd() as windows_sys::Win32::Foundation::HWND;
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
            let border_color: u32 = if dark { 0x00303031 } else { 0x00E7DED9 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                &border_color as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
            let dark_flag: i32 = if dark { 1 } else { 0 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                &dark_flag as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );
            let bg_color: u32 = if dark { 0x00121212 } else { 0x00F8F6F5 };
            let brush = CreateSolidBrush(bg_color);
            if !brush.is_null() {
                SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, brush as isize);
            }
        }
    }

    fn dispatch_ipc_cmd(proxy: &EventLoopProxy<UiWindowEvent>, msg: &str) {
        let parts: Vec<&str> = msg.trim().split(':').collect();
        let evt = match parts.as_slice() {
            ["drag_main"] => Some(UiWindowEvent::DragMain),
            ["minimize_main"] => Some(UiWindowEvent::MinimizeMain),
            ["close_main"] => Some(UiWindowEvent::CloseMain),
            ["show_main"] => Some(UiWindowEvent::ShowMain),
            ["show_main_qr"] => Some(UiWindowEvent::ShowMainQr),
            ["ready_main"] => Some(UiWindowEvent::ReadyMain),
            ["open_settings"] => Some(UiWindowEvent::OpenSettings),
            ["drag_settings"] => Some(UiWindowEvent::DragSettings),
            ["minimize_settings"] => Some(UiWindowEvent::MinimizeSettings),
            ["close_settings"] => Some(UiWindowEvent::CloseSettings),
            ["ready_settings"] => Some(UiWindowEvent::ReadySettings),
            ["ready_tray"] => Some(UiWindowEvent::ReadyTray),
            ["close_tray"] => Some(UiWindowEvent::CloseTray),
            ["ui_settings_changed"] => Some(UiWindowEvent::UiSettingsChanged),
            ["quit_app"] => Some(UiWindowEvent::QuitApp),
            ["theme", "dark"] => Some(UiWindowEvent::ThemeChanged(true)),
            ["theme", "light"] => Some(UiWindowEvent::ThemeChanged(false)),
            ["resize_main", w, h] => {
                if let (Ok(wf), Ok(hf)) = (w.parse::<f64>(), h.parse::<f64>()) {
                    Some(UiWindowEvent::ResizeMain(wf, hf))
                } else {
                    None
                }
            }
            ["resize_settings", w, h] => {
                if let (Ok(wf), Ok(hf)) = (w.parse::<f64>(), h.parse::<f64>()) {
                    Some(UiWindowEvent::ResizeSettings(wf, hf))
                } else {
                    None
                }
            }
            ["resize_tray", w, h] => {
                if let (Ok(wf), Ok(hf)) = (w.parse::<f64>(), h.parse::<f64>()) {
                    Some(UiWindowEvent::ResizeTray(wf, hf))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(e) = evt {
            let _ = proxy.send_event(e);
        }
    }

    fn attach_custom_protocol<'a>(
        builder: WebViewBuilder<'a>,
        rt: tokio::runtime::Handle,
        ipc: crate::server::DriverIpcClient,
        updater: crate::updater::DesktopUpdater,
    ) -> WebViewBuilder<'a> {
        builder.with_asynchronous_custom_protocol(
            "nfcbridge".into(),
            move |_webview_id, request, responder| {
                let method = request.method().as_str().to_string();
                let path = request
                    .uri()
                    .path_and_query()
                    .map(|pq| pq.as_str().to_string())
                    .unwrap_or_else(|| "/".to_string());
                let body = String::from_utf8_lossy(request.body()).to_string();
                let ipc_c = ipc.clone();
                let upd_c = updater.clone();
                rt.spawn(async move {
                    let (status_code, _status_text, content_type, resp_body) =
                        crate::ui_http::dispatch_ui_request(&method, &path, &body, &ipc_c, &upd_c)
                            .await;
                    let response = wry::http::Response::builder()
                        .status(status_code)
                        .header("Content-Type", content_type)
                        .header("Cache-Control", "no-store")
                        .header("Access-Control-Allow-Origin", "http://nfcbridge.localhost")
                        .body(Cow::Owned(resp_body.into_bytes()))
                        .unwrap_or_else(|_| {
                            wry::http::Response::builder()
                                .status(500)
                                .body(Cow::Borrowed(b"{}" as &[u8]))
                                .unwrap()
                        });
                    responder.respond(response);
                });
            },
        )
    }

    fn position_tray_popup(
        win: &Window,
        click_pos: PhysicalPosition<f64>,
        w: f64,
        h: f64,
    ) {
        let mon = win.current_monitor().or_else(|| win.primary_monitor());
        if let Some(mon) = mon {
            let sf = mon.scale_factor();
            let m_size = mon.size().to_logical::<f64>(sf);
            let m_pos = mon.position().to_logical::<f64>(sf);
            let click_log = click_pos.to_logical::<f64>(sf);

            let right_edge = m_pos.x + m_size.width - 12.0;
            let bottom_edge = m_pos.y + m_size.height - 56.0;
            let x = (click_log.x - w + 18.0).clamp(m_pos.x + 12.0, (right_edge - w).max(m_pos.x + 12.0));
            let y = (click_log.y - h - 12.0).clamp(m_pos.y + 12.0, (bottom_edge - h).max(m_pos.y + 12.0));
            win.set_outer_position(LogicalPosition::new(x, y));
        }
    }

    let event_loop = EventLoopBuilder::<UiWindowEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let icon_rgba = build_nfc_rgba_icon();
    let window_icon = icon_rgba
        .as_ref()
        .and_then(|(bytes, w, h)| Icon::from_rgba(bytes.clone(), *w, *h).ok());
    let tray_icon_img = icon_rgba
        .as_ref()
        .and_then(|(bytes, w, h)| tray_icon::Icon::from_rgba(bytes.clone(), *w, *h).ok());

    let webview_data_dir = data_dir.join("webview2");
    let _ = std::fs::create_dir_all(&webview_data_dir);
    let web_ctx: Rc<RefCell<WebContext>> =
        Rc::new(RefCell::new(WebContext::new(Some(webview_data_dir))));

    let init_dark = match crate::server::load_ui_settings().theme_mode.as_str() {
        "light" => false,
        _ => true,
    };
    let init_bg: (u8, u8, u8, u8) = if init_dark {
        (18, 18, 18, 255)
    } else {
        (245, 246, 248, 255)
    };

    let main_w = 860.0;
    let main_h = 566.0;
    let mut main_builder = WindowBuilder::new()
        .with_title(app_title)
        .with_decorations(false)
        .with_transparent(false)
        .with_visible(false)
        .with_resizable(false)
        .with_inner_size(LogicalSize::new(main_w, main_h));
    if let Some(ref ic) = window_icon {
        main_builder = main_builder.with_window_icon(Some(ic.clone()));
    }
    if let Some(mon) = event_loop.primary_monitor() {
        let sf = mon.scale_factor();
        let m_size = mon.size().to_logical::<f64>(sf);
        let m_pos = mon.position().to_logical::<f64>(sf);
        let x = m_pos.x + ((m_size.width - main_w) / 2.0).max(0.0);
        let y = m_pos.y + ((m_size.height - main_h) / 2.0).max(0.0);
        main_builder = main_builder.with_position(LogicalPosition::new(x, y));
    }

    let main_window = main_builder.build(&event_loop)?;
    apply_native_frameless_style(&main_window, init_dark);

    let main_url = "http://nfcbridge.localhost/?view=main";
    let ipc_proxy_main = proxy.clone();
    let main_webview = {
        let mut ctx = web_ctx.borrow_mut();
        let builder = WebViewBuilder::new_with_web_context(&mut ctx)
            .with_transparent(false)
            .with_background_color(init_bg)
            .with_url(main_url)
            .with_ipc_handler(move |req| {
                dispatch_ipc_cmd(&ipc_proxy_main, req.body());
            });
        attach_custom_protocol(builder, rt_handle.clone(), ipc.clone(), updater.clone())
            .build(&main_window)?
    };

    let fallback_proxy = proxy.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        let _ = fallback_proxy.send_event(UiWindowEvent::ReadyMain);
    });

    // Icono en bandeja del sistema (clic izquierdo abre ventana; clic derecho abre popup Stitch)
    let _tray = tray_icon_img.and_then(|ic| {
        TrayIconBuilder::new()
            .with_tooltip(app_title)
            .with_icon(ic)
            .build()
            .ok()
    });

    let proxy_tray = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if let TrayIconEvent::Click {
            button,
            button_state: MouseButtonState::Up,
            position,
            ..
        } = event
        {
            match button {
                MouseButton::Left => {
                    let _ = proxy_tray.send_event(UiWindowEvent::ShowMain);
                }
                MouseButton::Right => {
                    let _ = proxy_tray.send_event(UiWindowEvent::ToggleTrayPopup(position));
                }
                _ => {}
            }
        }
    }));

    let main_id = main_window.id();
    let mut main_shown = false;
    let mut current_dark = init_dark;
    let mut settings_pair: Option<(Window, WebView)> = None;
    let mut tray_pair: Option<(Window, WebView)> = None;
    let mut tray_size = (296.0_f64, 412.0_f64);
    let settings_title = format!("{} - Options", app_title);
    let tray_title = app_title.to_string();
    let settings_url = "http://nfcbridge.localhost/?view=settings";
    let tray_url = "http://nfcbridge.localhost/?view=tray";

    event_loop.run(move |event, elwt, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if window_id == main_id {
                    // Al cerrar la ventana principal se oculta al System Tray (no termina el proceso)
                    main_shown = false;
                    main_window.set_visible(false);
                } else if let Some((ref win, _)) = settings_pair {
                    if win.id() == window_id {
                        win.set_visible(false);
                    }
                } else if let Some((ref win, _)) = tray_pair {
                    if win.id() == window_id {
                        win.set_visible(false);
                    }
                }
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Focused(false),
                ..
            } => {
                if let Some((ref win, _)) = tray_pair {
                    if win.id() == window_id {
                        win.set_visible(false);
                    }
                }
            }
            Event::UserEvent(user_evt) => match user_evt {
                UiWindowEvent::DragMain => {
                    let _ = main_window.drag_window();
                }
                UiWindowEvent::MinimizeMain => {
                    main_window.set_minimized(true);
                }
                UiWindowEvent::ReadyMain => {
                    if !main_shown {
                        main_shown = true;
                        main_window.set_visible(true);
                        main_window.set_focus();
                    }
                }
                UiWindowEvent::ShowMain => {
                    if let Some((ref win, _)) = tray_pair {
                        win.set_visible(false);
                    }
                    main_shown = true;
                    main_window.set_visible(true);
                    main_window.set_minimized(false);
                    main_window.set_focus();
                    let _ = main_webview.evaluate_script(
                        "if (window.__nbRefreshNow) window.__nbRefreshNow();",
                    );
                }
                UiWindowEvent::ShowMainQr => {
                    if let Some((ref win, _)) = tray_pair {
                        win.set_visible(false);
                    }
                    main_shown = true;
                    main_window.set_visible(true);
                    main_window.set_minimized(false);
                    main_window.set_focus();
                    let _ = main_webview.evaluate_script(
                        "if (window.__nbOpenQuickQr) window.__nbOpenQuickQr();",
                    );
                }
                UiWindowEvent::CloseMain => {
                    // Cerrar la ventana principal minimiza al System Tray
                    main_shown = false;
                    main_window.set_visible(false);
                }
                UiWindowEvent::QuitApp => {
                    tray_pair = None;
                    settings_pair = None;
                    let _ = &main_webview;
                    *control_flow = ControlFlow::Exit;
                }
                UiWindowEvent::ResizeMain(w, h) => {
                    let clamped_w = w.clamp(800.0, 1280.0);
                    let clamped_h = h.clamp(420.0, 900.0);
                    main_window.set_inner_size(LogicalSize::new(clamped_w, clamped_h));
                }
                UiWindowEvent::UiSettingsChanged => {
                    let _ = main_webview.evaluate_script(
                        "if (window.__nbRefreshNow) window.__nbRefreshNow();",
                    );
                    if let Some((_, ref sview)) = settings_pair {
                        let _ = sview.evaluate_script(
                            "if (window.__nbRefreshNow) window.__nbRefreshNow();",
                        );
                    }
                    if let Some((_, ref tview)) = tray_pair {
                        let _ = tview.evaluate_script(
                            "if (window.__nbRefreshNow) window.__nbRefreshNow();",
                        );
                    }
                }
                UiWindowEvent::ThemeChanged(is_dark) => {
                    if current_dark != is_dark {
                        current_dark = is_dark;
                        apply_native_frameless_style(&main_window, is_dark);
                        if let Some((ref win, _)) = settings_pair {
                            apply_native_frameless_style(win, is_dark);
                        }
                        if let Some((ref win, _)) = tray_pair {
                            apply_native_frameless_style(win, is_dark);
                        }
                    }
                }
                UiWindowEvent::OpenSettings => {
                    if let Some((ref win, _)) = tray_pair {
                        win.set_visible(false);
                    }
                    if let Some((ref win, ref sview)) = settings_pair {
                        let _ = sview.evaluate_script(
                            "if (window.__nbRefreshNow) window.__nbRefreshNow();",
                        );
                        win.set_visible(true);
                        win.set_minimized(false);
                        win.set_focus();
                    } else {
                        let sw = 680.0;
                        let sh = 468.0;
                        let mut sb = WindowBuilder::new()
                            .with_title(&settings_title)
                            .with_decorations(false)
                            .with_transparent(false)
                            .with_visible(false)
                            .with_resizable(false)
                            .with_inner_size(LogicalSize::new(sw, sh));
                        if let Some(ref ic) = window_icon {
                            sb = sb.with_window_icon(Some(ic.clone()));
                        }
                        if let Ok(main_pos) = main_window.outer_position() {
                            let sf = main_window.scale_factor();
                            let mp = main_pos.to_logical::<f64>(sf);
                            let ms = main_window.inner_size().to_logical::<f64>(sf);
                            let x = mp.x + ((ms.width - sw) / 2.0);
                            let y = mp.y + ((ms.height - sh) / 2.0);
                            sb = sb.with_position(LogicalPosition::new(x.max(20.0), y.max(20.0)));
                        }
                        match sb.build(elwt) {
                            Ok(swin) => {
                                apply_native_frameless_style(&swin, current_dark);
                                let bg = if current_dark {
                                    (18, 18, 18, 255)
                                } else {
                                    (245, 246, 248, 255)
                                };
                                let ipc_proxy_s = proxy.clone();
                                let mut ctx = web_ctx.borrow_mut();
                                let builder = WebViewBuilder::new_with_web_context(&mut ctx)
                                    .with_transparent(false)
                                    .with_background_color(bg)
                                    .with_url(settings_url)
                                    .with_ipc_handler(move |req| {
                                        dispatch_ipc_cmd(&ipc_proxy_s, req.body());
                                    });
                                match attach_custom_protocol(
                                    builder,
                                    rt_handle.clone(),
                                    ipc.clone(),
                                    updater.clone(),
                                )
                                .build(&swin)
                                {
                                    Ok(sview) => {
                                        settings_pair = Some((swin, sview));
                                        let fallback_s = proxy.clone();
                                        std::thread::spawn(move || {
                                            std::thread::sleep(std::time::Duration::from_millis(800));
                                            let _ = fallback_s.send_event(UiWindowEvent::ReadySettings);
                                        });
                                    }
                                    Err(e) => warn!("No se pudo inicializar WebView de Opciones: {}", e),
                                }
                            }
                            Err(e) => warn!("No se pudo crear ventana de Opciones: {}", e),
                        }
                    }
                }
                UiWindowEvent::ReadySettings => {
                    if let Some((ref win, _)) = settings_pair {
                        if !win.is_visible() {
                            win.set_visible(true);
                            win.set_focus();
                        }
                    }
                }
                UiWindowEvent::DragSettings => {
                    if let Some((ref win, _)) = settings_pair {
                        let _ = win.drag_window();
                    }
                }
                UiWindowEvent::MinimizeSettings => {
                    if let Some((ref win, _)) = settings_pair {
                        win.set_minimized(true);
                    }
                }
                UiWindowEvent::CloseSettings => {
                    if let Some((ref win, _)) = settings_pair {
                        win.set_visible(false);
                    }
                }
                UiWindowEvent::ResizeSettings(w, h) => {
                    if let Some((ref win, _)) = settings_pair {
                        win.set_inner_size(LogicalSize::new(
                            w.clamp(540.0, 960.0),
                            h.clamp(380.0, 760.0),
                        ));
                    }
                }
                UiWindowEvent::ToggleTrayPopup(click_pos) => {
                    if let Some((ref win, ref tview)) = tray_pair {
                        if win.is_visible() {
                            win.set_visible(false);
                        } else {
                            position_tray_popup(win, click_pos, tray_size.0, tray_size.1);
                            let _ = tview.evaluate_script(
                                "if (window.__nbRefreshNow) window.__nbRefreshNow();",
                            );
                            win.set_visible(true);
                            win.set_focus();
                        }
                    } else {
                        let (tw, th) = tray_size;
                        let mut tb = WindowBuilder::new()
                            .with_title(&tray_title)
                            .with_decorations(false)
                            .with_transparent(false)
                            .with_visible(false)
                            .with_resizable(false)
                            .with_always_on_top(true)
                            .with_skip_taskbar(true)
                            .with_inner_size(LogicalSize::new(tw, th));
                        if let Some(ref ic) = window_icon {
                            tb = tb.with_window_icon(Some(ic.clone()));
                        }
                        match tb.build(elwt) {
                            Ok(twin) => {
                                apply_native_frameless_style(&twin, current_dark);
                                position_tray_popup(&twin, click_pos, tw, th);
                                let bg = if current_dark {
                                    (30, 30, 30, 255)
                                } else {
                                    (255, 255, 255, 255)
                                };
                                let ipc_proxy_t = proxy.clone();
                                let mut ctx = web_ctx.borrow_mut();
                                let builder = WebViewBuilder::new_with_web_context(&mut ctx)
                                    .with_transparent(false)
                                    .with_background_color(bg)
                                    .with_url(tray_url)
                                    .with_ipc_handler(move |req| {
                                        dispatch_ipc_cmd(&ipc_proxy_t, req.body());
                                    });
                                match attach_custom_protocol(
                                    builder,
                                    rt_handle.clone(),
                                    ipc.clone(),
                                    updater.clone(),
                                )
                                .build(&twin)
                                {
                                    Ok(tview) => {
                                        tray_pair = Some((twin, tview));
                                        let fallback_t = proxy.clone();
                                        std::thread::spawn(move || {
                                            std::thread::sleep(std::time::Duration::from_millis(600));
                                            let _ = fallback_t.send_event(UiWindowEvent::ReadyTray);
                                        });
                                    }
                                    Err(e) => warn!("No se pudo inicializar WebView del Tray: {}", e),
                                }
                            }
                            Err(e) => warn!("No se pudo crear popup del Tray: {}", e),
                        }
                    }
                }
                UiWindowEvent::ReadyTray => {
                    if let Some((ref win, _)) = tray_pair {
                        if !win.is_visible() {
                            win.set_visible(true);
                            win.set_focus();
                        }
                    }
                }
                UiWindowEvent::CloseTray => {
                    if let Some((ref win, _)) = tray_pair {
                        win.set_visible(false);
                    }
                }
                UiWindowEvent::ResizeTray(w, h) => {
                    let cw = w.clamp(280.0, 340.0);
                    let ch = h.clamp(240.0, 520.0);
                    let dh = ch - tray_size.1;
                    tray_size = (cw, ch);
                    if let Some((ref win, _)) = tray_pair {
                        if let Ok(pos) = win.outer_position() {
                            let sf = win.scale_factor();
                            let lp = pos.to_logical::<f64>(sf);
                            if dh.abs() > 1.0 {
                                win.set_outer_position(LogicalPosition::new(lp.x, (lp.y - dh).max(12.0)));
                            }
                        }
                        win.set_inner_size(LogicalSize::new(cw, ch));
                    }
                }
            },
            _ => {}
        }
    });
}
