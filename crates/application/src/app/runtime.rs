use iced::{Event, Subscription, Task, Theme, clipboard, event, keyboard};
use shared::AppContext;

use crate::subscriptions;

use super::message::Message;
use super::monitor::{detect_and_show_monitor_info, monitor_choices};
use super::overlay::{spawn_monitor_info_overlays, spawn_reward_overlay, spawn_test_overlay};
use super::state::Application;

pub fn run(context: &AppContext) -> iced::Result {
    let context = context.clone();

    iced::application(Application::title, Application::update, Application::view)
        .subscription(Application::subscription)
        .theme(|_| Theme::Dark)
        .run_with(move || {
            let mut application = Application::new(context);
            let startup_task = application.begin_startup_monitor_detection();
            let data_cache_task = application.begin_data_cache_refresh();
            (application, Task::batch([startup_task, data_cache_task]))
        })
}

impl Application {
    pub(super) fn title(&self) -> String {
        self.context.name().to_owned()
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        if !matches!(message, Message::UiEvent(_)) {
            log::debug!("iced application update event: {message:?}");
        }

        match message {
            Message::DetectMonitorInfo => {
                self.begin_monitor_detection();
                Task::perform(detect_and_show_monitor_info(), Message::MonitorInfoDetected)
            }
            Message::StartupMonitorInfoDetected(Ok(monitors)) => {
                if monitors.is_empty() {
                    self.finish_empty_monitor_detection();
                    return Task::none();
                }

                let choices = monitor_choices(monitors);
                self.finish_startup_monitor_detection(choices);
                Task::none()
            }
            Message::StartupMonitorInfoDetected(Err(err)) => {
                self.finish_failed_monitor_detection(err);
                Task::none()
            }
            Message::MonitorInfoDetected(Ok(monitors)) => {
                if monitors.is_empty() {
                    self.finish_empty_monitor_detection();
                    return Task::none();
                }

                let choices = monitor_choices(monitors);
                let overlay_result = spawn_monitor_info_overlays(&choices);
                self.finish_monitor_detection(choices, overlay_result);
                Task::none()
            }
            Message::MonitorInfoDetected(Err(err)) => {
                self.finish_failed_monitor_detection(err);
                Task::none()
            }
            Message::SelectedMonitorChanged(choice) => {
                self.select_monitor(choice);
                Task::none()
            }
            Message::DrawTestOverlay => {
                let selected_monitor = self.begin_test_overlay_launch();

                Task::perform(
                    async move { spawn_test_overlay(selected_monitor.as_ref()) },
                    Message::TestOverlayLaunched,
                )
            }
            Message::TestOverlayLaunched(result) => {
                self.record_test_overlay_launch(result);
                Task::none()
            }
            Message::QuitDebugOverlays => {
                let count = self.quit_debug_overlay_processes();
                self.report_closed_debug_overlays(count);
                Task::none()
            }
            Message::ToggleDiagnostics => {
                self.toggle_diagnostics();
                Task::none()
            }
            Message::ScanNow => self.trigger_manual_reward_scan(),
            Message::RefreshDataCache => self.begin_data_cache_refresh(),
            Message::DataCacheRefreshFinished(result) => {
                self.record_data_cache_refresh_finished(result);
                Task::none()
            }
            Message::ClipboardOutputChanged(enabled) => {
                self.set_clipboard_output_enabled(enabled);
                Task::none()
            }
            Message::OpenSettings => {
                self.open_settings_page();
                Task::none()
            }
            Message::CancelSettings => {
                self.cancel_settings_page();
                Task::none()
            }
            Message::SaveSettings => {
                self.save_settings_page();
                Task::none()
            }
            Message::SettingsTabSelected(tab) => {
                self.select_settings_tab(tab);
                Task::none()
            }
            Message::UiEvent(event) => {
                self.handle_ui_event(event);
                Task::none()
            }
            Message::SettingsAppLocaleChanged(value) => {
                self.set_settings_app_locale(value);
                Task::none()
            }
            Message::SettingsAppStartMinimizedChanged(value) => {
                self.set_settings_app_start_minimized(value);
                Task::none()
            }
            Message::SettingsCaptureMonitorChanged(value) => {
                self.set_settings_capture_monitor(value);
                Task::none()
            }
            Message::SettingsCaptureMethodChanged(value) => {
                self.set_settings_capture_method(value);
                Task::none()
            }
            Message::SettingsDisplayModeChanged(value) => {
                self.set_settings_display_mode(value);
                Task::none()
            }
            Message::SettingsAspectRatioChanged(value) => {
                self.set_settings_aspect_ratio(value);
                Task::none()
            }
            Message::SettingsScannerEnabledChanged(value) => {
                self.set_settings_scanner_enabled(value);
                Task::none()
            }
            Message::SettingsScannerAutoDelayChanged(value) => {
                self.set_settings_scanner_auto_delay(value);
                Task::none()
            }
            Message::SettingsScannerDebugImagesChanged(value) => {
                self.set_settings_scanner_debug_images(value);
                Task::none()
            }
            Message::SettingsScannerRetentionChanged(value) => {
                self.set_settings_scanner_retention(value);
                Task::none()
            }
            Message::StartHotkeyCapture(target) => {
                self.begin_hotkey_capture(target);
                Task::none()
            }
            Message::SettingsOverlayEnabledChanged(value) => {
                self.set_settings_overlay_enabled(value);
                Task::none()
            }
            Message::SettingsOverlayXOffsetChanged(value) => {
                self.set_settings_overlay_x_offset(value);
                Task::none()
            }
            Message::SettingsOverlayYOffsetChanged(value) => {
                self.set_settings_overlay_y_offset(value);
                Task::none()
            }
            Message::SettingsOverlayDurationChanged(value) => {
                self.set_settings_overlay_duration(value);
                Task::none()
            }
            Message::SettingsOverlayHighContrastChanged(value) => {
                self.set_settings_overlay_high_contrast(value);
                Task::none()
            }
            Message::SettingsClipboardEnabledChanged(value) => {
                self.set_settings_clipboard_enabled(value);
                Task::none()
            }
            Message::SettingsClipboardVaultedMarkerChanged(value) => {
                self.set_settings_clipboard_vaulted_marker(value);
                Task::none()
            }
            Message::SettingsClipboardFooterChanged(value) => {
                self.set_settings_clipboard_footer(value);
                Task::none()
            }
            Message::SettingsOcrLanguageChanged(value) => {
                self.set_settings_ocr_language(value);
                Task::none()
            }
            Message::SettingsTesseractDataPathChanged(value) => {
                self.set_settings_tesseract_data_path(value);
                Task::none()
            }
            Message::SettingsOcrConfidenceChanged(value) => {
                self.set_settings_ocr_confidence(value);
                Task::none()
            }
            Message::SettingsWarframeLogPathChanged(value) => {
                self.set_settings_warframe_log_path(value);
                Task::none()
            }
            Message::SettingsWarframeUiThemeChanged(value) => {
                self.set_settings_warframe_ui_theme(value);
                Task::none()
            }
            Message::SettingsLoggingLevelChanged(value) => {
                self.set_settings_logging_level(value);
                Task::none()
            }
            Message::SettingsLoggingFileChanged(value) => {
                self.set_settings_logging_file(value);
                Task::none()
            }
            Message::ServiceEvent(event) => self.handle_service_event(event),
            Message::RewardScanFinished(result) => {
                let overlay_result = result
                    .as_ref()
                    .ok()
                    .filter(|rewards| !rewards.is_empty())
                    .map(|rewards| spawn_reward_overlay(self.selected_monitor.as_ref(), rewards));
                let clipboard_summary = result.as_ref().ok().and_then(|rewards| {
                    shared::clipboard::reward_summary(&self.settings.clipboard, rewards)
                });
                let clipboard_queued = clipboard_summary.is_some();
                let clipboard_task = clipboard_summary
                    .map(clipboard::write)
                    .unwrap_or_else(Task::none);

                self.record_reward_scan_finished(result, overlay_result, clipboard_queued);
                clipboard_task
            }
        }
    }

    pub(super) fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            event::listen().map(Message::UiEvent),
            subscriptions::log_watcher::log_watcher_subscription(&self.settings)
                .map(Message::ServiceEvent),
            subscriptions::hotkey_watcher::hotkey_watcher_subscription(&self.settings)
                .map(Message::ServiceEvent),
        ])
    }

    fn handle_ui_event(&mut self, event: Event) {
        if self.capturing_hotkey.is_none() {
            return;
        }

        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
            return;
        };

        if matches!(
            key.as_ref(),
            keyboard::Key::Named(keyboard::key::Named::Escape)
        ) {
            self.cancel_hotkey_capture();
            return;
        }

        let Some(accelerator) = accelerator_from_key_press(&key, modifiers) else {
            return;
        };

        self.finish_hotkey_capture(accelerator);
    }
}

fn accelerator_from_key_press(
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<String> {
    let key = key_label(key)?;
    let mut parts = Vec::new();

    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if modifiers.logo() {
        parts.push("Super");
    }

    parts.push(&key);
    Some(parts.join("+"))
}

fn key_label(key: &keyboard::Key) -> Option<String> {
    match key.as_ref() {
        keyboard::Key::Character(character) => Some(character.to_ascii_uppercase()),
        keyboard::Key::Named(named) if is_modifier_key(named) => None,
        keyboard::Key::Named(named) => Some(format!("{named:?}")),
        keyboard::Key::Unidentified => None,
    }
}

fn is_modifier_key(key: keyboard::key::Named) -> bool {
    matches!(
        key,
        keyboard::key::Named::Alt
            | keyboard::key::Named::AltGraph
            | keyboard::key::Named::Control
            | keyboard::key::Named::Fn
            | keyboard::key::Named::FnLock
            | keyboard::key::Named::Hyper
            | keyboard::key::Named::Meta
            | keyboard::key::Named::Shift
            | keyboard::key::Named::Super
            | keyboard::key::Named::Symbol
            | keyboard::key::Named::SymbolLock
    )
}
