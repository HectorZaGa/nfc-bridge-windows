//! Host de escritorio nativo sin bordes (WebView2 transparente vía `tao` + `wry`)
//! para NFC Bridge Sync y NFC Bridge Tools Desktop.
//!
//! - Ventana principal sin barra de título nativa de Windows (`decorations: false`, `transparent: true`)
//!   con arrastre por cabecera, minimizar, cerrar y redimensionado exacto al contenedor HTML.
//! - Ventana de Opciones totalmente independiente (`?view=settings`) sin difuminar ni oscurecer
//!   la ventana principal.

#[cfg(target_os = "windows")]
pub fn run_desktop_ui(app_title: &str, ui_port: u16, data_dir: std::path::PathBuf) -> anyhow::Result<()> {
    use std::cell::RefCell;
    use std::rc::Rc;
    use tao::dpi::{LogicalPosition, LogicalSize};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
    use tao::window::{Icon, Window, WindowBuilder};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tracing::warn;
    use wry::{WebContext, WebView, WebViewBuilder};

    #[derive(Debug, Clone)]
    enum UiWindowEvent {
        DragMain,
        MinimizeMain,
        CloseMain,
        ShowMain,
        ResizeMain(f64, f64),
        OpenSettings,
        DragSettings,
        MinimizeSettings,
        CloseSettings,
        ResizeSettings(f64, f64),
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
                // Fondo azul eléctrico con esquinas redondeadas
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

    fn dispatch_ipc_cmd(proxy: &EventLoopProxy<UiWindowEvent>, msg: &str) {
        let parts: Vec<&str> = msg.trim().split(':').collect();
        let evt = match parts.as_slice() {
            ["drag_main"] => Some(UiWindowEvent::DragMain),
            ["minimize_main"] => Some(UiWindowEvent::MinimizeMain),
            ["close_main"] => Some(UiWindowEvent::CloseMain),
            ["open_settings"] => Some(UiWindowEvent::OpenSettings),
            ["drag_settings"] => Some(UiWindowEvent::DragSettings),
            ["minimize_settings"] => Some(UiWindowEvent::MinimizeSettings),
            ["close_settings"] => Some(UiWindowEvent::CloseSettings),
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
            _ => None,
        };
        if let Some(e) = evt {
            let _ = proxy.send_event(e);
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
    let web_ctx: Rc<RefCell<WebContext>> = Rc::new(RefCell::new(WebContext::new(Some(webview_data_dir))));

    let main_w = 860.0;
    let main_h = 566.0;
    let mut main_builder = WindowBuilder::new()
        .with_title(app_title)
        .with_decorations(false)
        .with_transparent(true)
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
    let main_url = format!("http://127.0.0.1:{}/?view=main", ui_port);
    let ipc_proxy_main = proxy.clone();
    let main_webview = {
        let mut ctx = web_ctx.borrow_mut();
        WebViewBuilder::new_with_web_context(&mut ctx)
            .with_transparent(true)
            .with_background_color((0, 0, 0, 0))
            .with_url(&main_url)
            .with_ipc_handler(move |req| {
                dispatch_ipc_cmd(&ipc_proxy_main, req.body());
            })
            .build(&main_window)?
    };

    // Bandeja del sistema opcional (abrir, opciones, salir)
    let tray_menu = Menu::new();
    let item_open = MenuItem::new(app_title, true, None);
    let item_opts = MenuItem::new("Options / Opciones", true, None);
    let item_quit = MenuItem::new("Exit / Salir", true, None);
    let open_id = item_open.id().clone();
    let opts_id = item_opts.id().clone();
    let quit_id = item_quit.id().clone();
    let _ = tray_menu.append(&item_open);
    let _ = tray_menu.append(&item_opts);
    let _ = tray_menu.append(&item_quit);

    let _tray = tray_icon_img.and_then(|ic| {
        TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip(app_title)
            .with_icon(ic)
            .build()
            .ok()
    });

    let proxy_tray = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let _ = proxy_tray.send_event(UiWindowEvent::ShowMain);
        }
    }));

    let proxy_menu = proxy.clone();
    MenuEvent::set_event_handler(Some(std::sync::Arc::new(move |event: MenuEvent| {
        if event.id == open_id {
            let _ = proxy_menu.send_event(UiWindowEvent::ShowMain);
        } else if event.id == opts_id {
            let _ = proxy_menu.send_event(UiWindowEvent::OpenSettings);
        } else if event.id == quit_id {
            let _ = proxy_menu.send_event(UiWindowEvent::QuitApp);
        }
    })));

    let main_id = main_window.id();
    let mut settings_pair: Option<(Window, WebView)> = None;
    let settings_title = format!("{} - Options", app_title);
    let settings_url = format!("http://127.0.0.1:{}/?view=settings", ui_port);

    event_loop.run(move |event, elwt, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if window_id == main_id {
                    settings_pair = None;
                    *control_flow = ControlFlow::Exit;
                } else if settings_pair.as_ref().is_some_and(|(w, _)| w.id() == window_id) {
                    settings_pair = None;
                }
            }
            Event::UserEvent(user_evt) => match user_evt {
                UiWindowEvent::DragMain => {
                    let _ = main_window.drag_window();
                }
                UiWindowEvent::MinimizeMain => {
                    main_window.set_minimized(true);
                }
                UiWindowEvent::ShowMain => {
                    main_window.set_visible(true);
                    main_window.set_minimized(false);
                    main_window.set_focus();
                }
                UiWindowEvent::CloseMain | UiWindowEvent::QuitApp => {
                    settings_pair = None;
                    let _ = &main_webview;
                    *control_flow = ControlFlow::Exit;
                }
                UiWindowEvent::ResizeMain(w, h) => {
                    let clamped_w = w.clamp(800.0, 1280.0);
                    let clamped_h = h.clamp(420.0, 900.0);
                    main_window.set_inner_size(LogicalSize::new(clamped_w, clamped_h));
                }
                UiWindowEvent::OpenSettings => {
                    if let Some((ref win, _)) = settings_pair {
                        win.set_visible(true);
                        win.set_minimized(false);
                        win.set_focus();
                    } else {
                        let sw = 680.0;
                        let sh = 468.0;
                        let mut sb = WindowBuilder::new()
                            .with_title(&settings_title)
                            .with_decorations(false)
                            .with_transparent(true)
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
                                let ipc_proxy_s = proxy.clone();
                                let mut ctx = web_ctx.borrow_mut();
                                match WebViewBuilder::new_with_web_context(&mut ctx)
                                    .with_transparent(true)
                                    .with_background_color((0, 0, 0, 0))
                                    .with_url(&settings_url)
                                    .with_ipc_handler(move |req| {
                                        dispatch_ipc_cmd(&ipc_proxy_s, req.body());
                                    })
                                    .build(&swin)
                                {
                                    Ok(sview) => {
                                        swin.set_focus();
                                        settings_pair = Some((swin, sview));
                                    }
                                    Err(e) => warn!("No se pudo inicializar WebView de Opciones: {}", e),
                                }
                            }
                            Err(e) => warn!("No se pudo crear ventana de Opciones: {}", e),
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
                    settings_pair = None;
                }
                UiWindowEvent::ResizeSettings(w, h) => {
                    if let Some((ref win, _)) = settings_pair {
                        win.set_inner_size(LogicalSize::new(w.clamp(540.0, 960.0), h.clamp(380.0, 760.0)));
                    }
                }
            },
            _ => {}
        }
    });
}
