use shared::AppContext;
use shared::config::Settings as AppSettings;

use super::monitor::MonitorChoice;
use super::settings::{Page, SettingsTab};

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
    pub(super) page: Page,
    pub(super) settings_tab: SettingsTab,
    pub(super) settings_draft: AppSettings,
}

impl Application {
    pub(super) fn new(context: AppContext) -> Self {
        let (settings, status) = match context.load_settings() {
            Ok(settings) => (
                settings,
                "Ready. Use the debug buttons to query monitor streams or draw overlays."
                    .to_owned(),
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
            page: Page::Launcher,
            settings_tab: SettingsTab::App,
            settings_draft,
        }
    }
}
