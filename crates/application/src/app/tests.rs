use shared::{AppContext, monitor::MonitorInfo, rewards::RewardOverlayEntry};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::monitor::monitor_choices;
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

    application.record_reward_scan_finished(Ok(rewards), Some(Ok(123)));

    assert_eq!(application.overlay_processes, vec![123]);
    assert_eq!(
        application.status,
        "Found 2 reward(s) and sent them to the overlay."
    );
}

#[test]
fn reward_scan_success_without_rewards_does_not_launch_overlay() {
    let mut application = Application::new(test_context());

    application.record_reward_scan_finished(Ok(Vec::new()), None);

    assert!(application.overlay_processes.is_empty());
    assert_eq!(
        application.status,
        "Reward scan completed, but no rewards were found."
    );
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
