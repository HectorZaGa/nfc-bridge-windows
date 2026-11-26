use super::theme::ThemePalette;
use crate::i18n::I18nCatalog;
use crate::ipc::{DesktopLiveState, DriverIpcClient, IpcCommand};
use crate::updater::{DesktopUpdater, UpdatesOverviewState};
use egui::{Color32, Pos2};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct IssuedQrView {
    pub(crate) method: String,
    pub(crate) pair_token: String,
    pub(crate) expires_at: Instant,
    pub(crate) uri: String,
    pub(crate) share_link: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OptionsTab {
    Appearance,
    Network,
    Updates,
    About,
}

pub(crate) struct SharedUiCore {
    pub(crate) ipc: DriverIpcClient,
    pub(crate) updater: DesktopUpdater,
    pub(crate) rt: tokio::runtime::Handle,
    pub(crate) i18n: I18nCatalog,
    pub(crate) app_title: String,

    pub(crate) live: DesktopLiveState,
    pub(crate) updates: UpdatesOverviewState,

    // Control de ventanas
    pub(crate) main_visible: bool,
    pub(crate) main_want_focus: bool,
    pub(crate) options_open: bool,
    pub(crate) options_just_opened: bool,
    pub(crate) options_want_focus: bool,
    pub(crate) tray_open: bool,
    pub(crate) tray_opened_at: Option<Instant>,
    pub(crate) tray_pos: Option<Pos2>,
    pub(crate) tray_devices_expanded: bool,
    pub(crate) should_quit: bool,

    // Estado visual de la ventana principal
    pub(crate) active_panel: String,
    pub(crate) qr_method: Option<String>,
    pub(crate) qr_data: Option<IssuedQrView>,
    pub(crate) qr_error: Option<String>,
    pub(crate) qr_loading: bool,
    pub(crate) live_relay_link: Option<String>,
    pub(crate) live_relay_expires_at: Option<Instant>,

    // Perfil / dropdown / renombrado inline
    pub(crate) device_dropdown_open: bool,
    pub(crate) editing_device_id: Option<String>,
    pub(crate) editing_device_name: String,

    // Inputs y feedback con protección anti-sobrescritura mientras se editan/guardan
    pub(crate) relay_url_buf: String,
    pub(crate) relay_url_inited: bool,

    pub(crate) options_tab: OptionsTab,
    pub(crate) sidebar_compact: bool,
    pub(crate) language_dropdown_open: bool,

    pub(crate) net_ip_buf: String,
    pub(crate) net_port_buf: String,
    pub(crate) net_dirty_until: Option<Instant>,
    pub(crate) net_feedback: Option<(String, bool, Instant)>,

    pub(crate) ui_settings_dirty_until: Option<Instant>,
    pub(crate) snapshot_dirty_until: Option<Instant>,
    pub(crate) auto_mobile_dirty_until: Option<Instant>,
    pub(crate) auto_mobile_optimistic: bool,

    // Selector HSV flotante
    pub(crate) color_picker_open: bool,
    pub(crate) color_picker_anchor: Pos2,
    pub(crate) hsv_h: f32,
    pub(crate) hsv_s: f32,
    pub(crate) hsv_v: f32,
    pub(crate) hsv_hex_buf: String,
    pub(crate) recent_accents: Vec<String>,

    // Misc y caché DWM/Visuals
    pub(crate) copied_key: Option<(String, Instant)>,
    pub(crate) upd_spin_until: Option<Instant>,
    pub(crate) restart_dismissed: bool,
    pub(crate) last_dwm_main_key: Option<(bool, Color32)>,
    pub(crate) last_dwm_opts_key: Option<(bool, Color32)>,
    pub(crate) last_dwm_tray_key: Option<(bool, Color32)>,
    pub(crate) last_visuals_pal: Option<ThemePalette>,
}

impl SharedUiCore {
    pub(crate) fn current_lang(&self) -> &str {
        &self.live.ui_settings.language
    }

    pub(crate) fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.i18n.t(self.current_lang(), key)
    }

    pub(crate) fn t_vars(&self, key: &str, vars: &[(&str, &str)]) -> String {
        self.i18n.t_vars(self.current_lang(), key, vars)
    }

    pub(crate) fn palette(&self) -> ThemePalette {
        ThemePalette::from_settings(&self.live.ui_settings, &self.live.windows_accent_hex)
    }

    pub(crate) fn app_display_name(&self) -> String {
        if !self.updates.app_display_name.is_empty() {
            self.updates.app_display_name.clone()
        } else {
            let key = if self.updates.app_product == "tools" {
                "app_tools_name"
            } else {
                "app_sync_name"
            };
            self.t(key).to_string()
        }
    }

    pub(crate) fn has_pending_update(&self) -> bool {
        self.updates.app_status == "available"
            || self.updates.app_status == "installed_restart"
            || self.updates.driver_status == "available"
    }

    pub(crate) fn persist_ui_settings(&mut self) {
        self.ui_settings_dirty_until = Some(Instant::now() + Duration::from_millis(1800));
        let settings = self.live.ui_settings.clone();
        let ipc = self.ipc.clone();
        self.rt.spawn(async move {
            ipc.update_ui_settings(settings).await;
        });
    }

    pub(crate) fn send_ipc(&mut self, cmd: IpcCommand) {
        let mut applied_optimistic = true;
        match &cmd {
            IpcCommand::SetUsbEnabled { enabled } => self.live.snapshot.usb_enabled = *enabled,
            IpcCommand::SetUsbPriority { enabled } => self.live.snapshot.usb_auto_connect = *enabled,
            IpcCommand::SetTcpEnabled { enabled } => self.live.snapshot.tcp_enabled = *enabled,
            IpcCommand::SetBluetoothEnabled { enabled } => self.live.snapshot.bt_enabled = *enabled,
            IpcCommand::SetWifiDirectEnabled { enabled } => self.live.snapshot.wfd_enabled = *enabled,
            IpcCommand::SetPaused { paused } => self.live.paused = *paused,
            IpcCommand::SetAutoAcceptMobileSwitch { enabled } => {
                self.live.snapshot.auto_accept_mobile_switch = *enabled;
            }
            _ => applied_optimistic = false,
        }
        if applied_optimistic {
            self.snapshot_dirty_until = Some(Instant::now() + Duration::from_millis(900));
        }

        let ipc = self.ipc.clone();
        self.rt.spawn(async move {
            let _ = ipc.send_command(cmd).await;
            let _ = ipc.send_command(IpcCommand::GetTransportSnapshot).await;
        });
    }
}
