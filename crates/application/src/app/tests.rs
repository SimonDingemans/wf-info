use shared::{
    AppContext,
    monitor::MonitorInfo,
    rewards::RewardOverlayEntry,
    watchers::{
        events::ServiceEvent,
        log_watcher::{LogFileSelection, LogFileSelectionSource, LogWatcherEvent},
    },
};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::data_cache::{DataCacheRefresh, DataCacheSource, DataPayloadRefresh};

use super::monitor::monitor_choices;
use super::settings::{HotkeyCaptureTarget, Page, SettingsTab};
use super::state::Application;

#[test]
fn monitor_detection_preserves_selected_output_when_available() {
    let mut application = Application::new(test_context());
    let original_choices = monitor_choices(vec![monitor_info("DP-1"), monitor_info("HDMI-A-1")]);
    application.finish_monitor_detection(original_choices.clone(), Ok(Vec::new()));
    application.select_monitor(original_choices[1].clone());

    let refreshed_choices = monitor_choices(vec![monitor_info("DP-1"), monitor_info("HDMI-A-1")]);
    application.finish_monitor_detection(refreshed_choices.clone(), Ok(Vec::new()));

    assert_eq!(
        application
            .selected_monitor
            .as_ref()
            .map(|choice| choice.label.as_str()),
        Some("2: HDMI-A-1")
    );
    assert_eq!(application.monitors, refreshed_choices);
}

#[test]
fn empty_monitor_detection_clears_previous_selection() {
    let mut application = Application::new(test_context());
    let choices = monitor_choices(vec![monitor_info("DP-1")]);
    application.finish_monitor_detection(choices, Ok(vec![42]));

    application.finish_empty_monitor_detection();

    assert!(application.monitors.is_empty());
    assert_eq!(application.selected_monitor, None);
    assert!(application.overlay_processes.contains(&42));
    assert!(!application.busy);
}

#[test]
fn test_overlay_launch_records_success_and_failure_through_application_api() {
    let mut application = Application::new(test_context());

    application.record_test_overlay_launch(Ok(99));

    assert_eq!(application.overlay_processes, vec![99]);
    assert_eq!(application.status, "Test overlay launched.");

    application.record_test_overlay_launch(Err("boom".to_owned()));

    assert_eq!(application.overlay_processes, vec![99]);
    assert_eq!(application.status, "Could not launch test overlay: boom");
}

#[test]
fn reward_scan_success_launches_overlay_when_rewards_are_found() {
    let mut application = Application::new(test_context());
    let rewards = vec![
        RewardOverlayEntry::name_only("Forma Blueprint"),
        RewardOverlayEntry::name_only("Braton Prime Receiver"),
    ];

    application.record_reward_scan_finished(Ok(rewards), Some(Ok(123)), false);

    assert_eq!(application.overlay_processes, vec![123]);
    assert_eq!(
        application.status,
        "Found 2 reward(s) and sent them to the overlay."
    );
}

#[test]
fn reward_scan_success_reports_queued_clipboard_summary() {
    let mut application = Application::new(test_context());
    let rewards = vec![RewardOverlayEntry::name_only("Forma Blueprint").with_platinum(12)];

    application.record_reward_scan_finished(Ok(rewards), None, true);

    assert_eq!(
        application.status,
        "Found 1 reward(s). Clipboard summary queued."
    );
}

#[test]
fn enabled_clipboard_settings_format_reward_scan_summary() {
    let mut application = Application::new(test_context());
    application.settings.clipboard.enabled = true;
    application.settings.clipboard.footer = "via wf-info".to_owned();
    let rewards = vec![RewardOverlayEntry::name_only("Forma Blueprint").with_platinum(12)];

    let summary = shared::clipboard::reward_summary(&application.settings.clipboard, &rewards);

    assert_eq!(
        summary,
        Some("Forma Blueprint: 12p\nvia wf-info".to_owned())
    );
}

#[test]
fn reward_scan_success_without_rewards_does_not_launch_overlay() {
    let mut application = Application::new(test_context());

    application.record_reward_scan_finished(Ok(Vec::new()), None, false);

    assert!(application.overlay_processes.is_empty());
    assert_eq!(
        application.status,
        "Reward scan completed, but no rewards were found."
    );
}

#[test]
fn clipboard_output_setting_is_persisted() {
    let mut application = Application::new(test_context());

    application.set_clipboard_output_enabled(true);

    assert!(application.settings.clipboard.enabled);
    assert_eq!(application.status, "Clipboard summaries enabled.");

    let saved = application.context.load_settings().expect("saved settings");
    assert!(saved.clipboard.enabled);
}

#[test]
fn settings_page_save_persists_draft_and_returns_to_launcher() {
    let mut application = Application::new(test_context());

    application.open_settings_page();
    application.set_settings_app_locale("nl".to_owned());
    application.set_settings_clipboard_enabled(true);
    application.save_settings_page();

    assert_eq!(application.page, Page::Launcher);
    assert_eq!(application.status, "Settings saved.");
    assert_eq!(application.settings.app.locale, "nl");
    assert!(application.settings.clipboard.enabled);

    let saved = application.context.load_settings().expect("saved settings");
    assert_eq!(saved.app.locale, "nl");
    assert!(saved.clipboard.enabled);
}

#[test]
fn settings_page_cancel_discards_draft_changes() {
    let mut application = Application::new(test_context());

    application.open_settings_page();
    application.set_settings_app_locale("nl".to_owned());
    application.cancel_settings_page();

    assert_eq!(application.page, Page::Launcher);
    assert_eq!(application.status, "Settings changes cancelled.");
    assert_eq!(application.settings.app.locale, "en");
    assert_eq!(application.settings_draft.app.locale, "en");
}

#[test]
fn settings_page_rejects_unsupported_capture_values() {
    let mut application = Application::new(test_context());

    application.open_settings_page();
    application.set_settings_display_mode("windowed".to_owned());
    application.save_settings_page();

    assert_eq!(application.page, Page::Settings);
    assert_eq!(application.settings_tab, SettingsTab::Capture);
    assert!(application.status.contains("borderless_fullscreen"));
    assert_eq!(
        application.settings.capture.display_mode,
        "borderless_fullscreen"
    );
}

#[test]
fn settings_hotkey_capture_updates_targeted_draft_hotkey() {
    let mut application = Application::new(test_context());

    application.begin_hotkey_capture(HotkeyCaptureTarget::Activation);
    application.finish_hotkey_capture("Ctrl+Shift+F12".to_owned());

    assert_eq!(application.capturing_hotkey, None);
    assert_eq!(
        application.settings_draft.hotkeys.activation,
        "Ctrl+Shift+F12"
    );
    assert_eq!(application.settings.hotkeys.activation, "F12");
}

#[test]
fn settings_hotkey_capture_can_be_cancelled() {
    let mut application = Application::new(test_context());

    application.begin_hotkey_capture(HotkeyCaptureTarget::DismissOverlay);
    application.cancel_hotkey_capture();

    assert_eq!(application.capturing_hotkey, None);
    assert_eq!(application.status, "Hotkey capture cancelled.");
    assert_eq!(application.settings_draft.hotkeys.dismiss_overlay, "F11");
}

#[test]
fn data_cache_refresh_records_success_and_failure() {
    let mut application = Application::new(test_context());
    let _ = application.begin_data_cache_refresh();

    application.record_data_cache_refresh_finished(Ok(DataCacheRefresh {
        prices: remote_payload("/tmp/wf-info/prices.json"),
        filtered_items: remote_payload("/tmp/wf-info/filtered_items.json"),
        warframe_market_items: remote_payload("/tmp/wf-info/warframe_market_items.json"),
    }));

    assert!(!application.data_cache_refresh_in_progress);
    assert_eq!(
        application.status,
        "Data cache ready: 3 remote payload(s), 0 local fallback payload(s)."
    );

    let _ = application.begin_data_cache_refresh();
    application.record_data_cache_refresh_finished(Err("offline".to_owned()));

    assert!(!application.data_cache_refresh_in_progress);
    assert_eq!(application.status, "Data cache refresh failed: offline");
}

#[test]
fn selected_monitor_overrides_configured_capture_monitor_for_reward_scans() {
    let mut application = Application::new(test_context());
    application.settings.set_capture_monitor("primary");
    let choices = monitor_choices(vec![monitor_info("DP-1"), monitor_info("DP-3")]);
    application.finish_monitor_detection(choices.clone(), Ok(Vec::new()));
    application.select_monitor(choices[1].clone());

    let settings = application.reward_scan_settings();

    assert_eq!(settings.capture.monitor, "DP-3");
}

#[test]
fn startup_monitor_detection_populates_choices_without_launching_overlays() {
    let mut application = Application::new(test_context());
    let choices = monitor_choices(vec![monitor_info("DP-1"), monitor_info("DP-3")]);

    application.finish_startup_monitor_detection(choices.clone());

    assert_eq!(application.monitors, choices);
    assert_eq!(
        application
            .selected_monitor
            .as_ref()
            .map(|choice| choice.output_name.as_deref()),
        Some(Some("DP-1"))
    );
    assert!(application.overlay_processes.is_empty());
    assert_eq!(application.status, "Detected 2 connected monitor(s).");
}

#[test]
fn application_title_uses_app_name_without_debug_shell_suffix() {
    let application = Application::new(test_context());

    assert_eq!(application.title(), "wf-info-test");
}

#[test]
fn manual_scan_sets_in_progress_state() {
    let mut application = Application::new(test_context());

    let _ = application.trigger_manual_reward_scan();

    assert!(application.reward_scan_in_progress);
    assert_eq!(application.status, "Reward scan started.");
    assert!(application.last_reward_scan.is_none());
}

#[test]
fn disabled_scanner_prevents_manual_scan() {
    let mut application = Application::new(test_context());
    application.settings.scanner.enabled = false;

    let _ = application.trigger_manual_reward_scan();

    assert!(!application.reward_scan_in_progress);
    assert_eq!(
        application.status,
        "Reward scanner is disabled. Enable it in settings to scan now."
    );
    assert!(application.last_reward_scan.is_none());
}

#[test]
fn reward_scan_completion_records_last_scan_result() {
    let mut application = Application::new(test_context());
    let rewards = vec![RewardOverlayEntry::name_only("Forma Blueprint").with_platinum(12)];

    application.record_reward_scan_finished(Ok(rewards.clone()), None, false);

    assert_eq!(application.last_reward_scan, Some(Ok(rewards)));

    application.record_reward_scan_finished(Err("ocr failed".to_owned()), None, false);

    assert_eq!(
        application.last_reward_scan,
        Some(Err("ocr failed".to_owned()))
    );
}

#[test]
fn data_cache_refresh_completion_records_last_result() {
    let mut application = Application::new(test_context());
    let refresh = DataCacheRefresh {
        prices: remote_payload("/tmp/wf-info/prices.json"),
        filtered_items: local_fallback_payload("/tmp/wf-info/filtered_items.json"),
        warframe_market_items: remote_payload("/tmp/wf-info/warframe_market_items.json"),
    };

    application.record_data_cache_refresh_finished(Ok(refresh.clone()));

    assert_eq!(application.last_data_cache_refresh, Some(Ok(refresh)));

    application.record_data_cache_refresh_finished(Err("offline".to_owned()));

    assert_eq!(
        application.last_data_cache_refresh,
        Some(Err("offline".to_owned()))
    );
}

#[test]
fn diagnostics_toggle_changes_expanded_state() {
    let mut application = Application::new(test_context());

    application.toggle_diagnostics();
    assert!(application.diagnostics_expanded);

    application.toggle_diagnostics();
    assert!(!application.diagnostics_expanded);
}

#[test]
fn log_watcher_selection_updates_application_status() {
    let mut application = Application::new(test_context());

    let _ = application.handle_service_event(ServiceEvent::LogWatcher(
        LogWatcherEvent::LogFileSelected(LogFileSelection {
            path: "/tmp/EE.log".into(),
            source: LogFileSelectionSource::Discovered,
        }),
    ));

    assert_eq!(application.status, "Discovered Warframe log: /tmp/EE.log");
}

fn test_context() -> AppContext {
    static NEXT_CONFIG_ID: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT_CONFIG_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "wf-info-application-test-{}-{id}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    AppContext::new("wf-info-test").with_config_path(path)
}

fn remote_payload(path: &str) -> DataPayloadRefresh {
    DataPayloadRefresh {
        path: path.into(),
        source: DataCacheSource::Remote,
    }
}

fn local_fallback_payload(path: &str) -> DataPayloadRefresh {
    DataPayloadRefresh {
        path: path.into(),
        source: DataCacheSource::LocalFallback,
    }
}

fn monitor_info(name: &str) -> MonitorInfo {
    MonitorInfo {
        pipe_wire_node_id: None,
        id: Some(name.to_owned()),
        mapping_id: None,
        position: Some((0, 0)),
        size: Some((1920, 1080)),
        source_type: Some("test".to_owned()),
        xdg_output_name: Some(name.to_owned()),
    }
}
