use shared::AppContext;
use shared::config::Settings as AppSettings;
use shared::rewards::RewardOverlayEntry;

use crate::data_cache::DataCacheRefresh;

use super::monitor::MonitorChoice;
use super::settings::{HotkeyCaptureTarget, Page, SettingsTab};

#[derive(Debug)]
pub(super) struct Application {
    pub(super) context: AppContext,
    pub(super) settings: AppSettings,
    pub(super) monitors: Vec<MonitorChoice>,
    pub(super) selected_monitor: Option<MonitorChoice>,
    pub(super) overlay_processes: Vec<u32>,
    pub(super) status: String,
    pub(super) busy: bool,
    pub(super) data_cache_refresh_in_progress: bool,
    pub(super) reward_scan_in_progress: bool,
    pub(super) diagnostics_expanded: bool,
    pub(super) last_data_cache_refresh: Option<Result<DataCacheRefresh, String>>,
    pub(super) last_reward_scan: Option<Result<Vec<RewardOverlayEntry>, String>>,
    pub(super) page: Page,
    pub(super) settings_tab: SettingsTab,
    pub(super) settings_draft: AppSettings,
    pub(super) capturing_hotkey: Option<HotkeyCaptureTarget>,
}

impl Application {
    pub(super) fn new(context: AppContext) -> Self {
        let (settings, status) = match context.load_settings() {
            Ok(settings) => (
                settings,
                "Ready. Reward scanner dashboard initialized.".to_owned(),
            ),
            Err(err) => {
                log::warn!("failed to load settings, using defaults: {err}");
                (
                    AppSettings::default(),
                    format!("Settings could not be loaded, using defaults: {err}"),
                )
            }
        };

        let settings_draft = settings.clone();

        Self {
            context,
            settings,
            monitors: Vec::new(),
            selected_monitor: None,
            overlay_processes: Vec::new(),
            status,
            busy: false,
            data_cache_refresh_in_progress: false,
            reward_scan_in_progress: false,
            diagnostics_expanded: false,
            last_data_cache_refresh: None,
            last_reward_scan: None,
            page: Page::Launcher,
            settings_tab: SettingsTab::App,
            settings_draft,
            capturing_hotkey: None,
        }
    }
}
