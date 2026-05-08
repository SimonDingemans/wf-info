use std::{path::PathBuf, process::Command as ProcessCommand};

use crate::reward_scan::{RewardScanTrigger, scan_rewards_for_overlay};
use crate::subscriptions;
use iced::widget::{button, column, container, pick_list, row, scrollable, text};
use iced::{Element, Length, Subscription, Task, Theme};
use shared::config::Settings as AppSettings;
use shared::rewards::RewardOverlayEntry;
use shared::watchers::events::{ServiceEvent, WatcherEvent, WatcherKind};
use shared::watchers::hotkey_watcher::{HotkeyAction, HotkeyEvent};
use shared::watchers::log_watcher::LogWatcherEvent;
use shared::{AppContext, monitor};

pub fn run(context: &AppContext) -> iced::Result {
    let context = context.clone();

    iced::application(Application::title, Application::update, Application::view)
        .subscription(Application::subscription)
        .theme(|_| Theme::Dark)
        .run_with(move || {
            let mut application = Application::new(context);
            let startup_task = application.begin_startup_monitor_detection();
            (application, startup_task)
        })
}

#[derive(Debug)]
struct Application {
    context: AppContext,
    settings: AppSettings,
    monitors: Vec<MonitorChoice>,
    selected_monitor: Option<MonitorChoice>,
    overlay_processes: Vec<u32>,
    status: String,
    busy: bool,
}

impl Application {
    fn title(&self) -> String {
        format!("{} debug shell", self.context.name())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        log::debug!("iced application update event: {message:?}");

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
            Message::ServiceEvent(event) => self.handle_service_event(event),
            Message::RewardScanFinished(result) => {
                let overlay_result = result
                    .as_ref()
                    .ok()
                    .filter(|rewards| !rewards.is_empty())
                    .map(|rewards| spawn_reward_overlay(self.selected_monitor.as_ref(), rewards));

                self.record_reward_scan_finished(result, overlay_result);
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            subscriptions::log_watcher::log_watcher_subscription(&self.settings)
                .map(Message::ServiceEvent),
            subscriptions::hotkey_watcher::hotkey_watcher_subscription(&self.settings)
                .map(Message::ServiceEvent),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let detect_button = button("Show Monitor Info Overlays")
            .padding([10, 14])
            .on_press_maybe((!self.busy).then_some(Message::DetectMonitorInfo));

        let test_overlay_button = button("Draw Test Overlay")
            .padding([10, 14])
            .on_press(Message::DrawTestOverlay);
        let quit_overlays_button = button("Quit Debug Overlays")
            .padding([10, 14])
            .on_press(Message::QuitDebugOverlays);

        let selected = self.selected_monitor.clone();
        let monitor_picker = pick_list(
            self.monitors.as_slice(),
            selected,
            Message::SelectedMonitorChanged,
        )
        .placeholder("Active screen");

        let monitor_rows = self
            .monitors
            .iter()
            .fold(column![].spacing(8), |column, monitor| {
                let details = monitor
                    .info
                    .summary_lines()
                    .into_iter()
                    .fold(column![].spacing(2), |column, line| column.push(text(line)));

                column.push(container(details).padding(12).width(Length::Fill))
            });

        let content = column![
            text("wf-info").size(32),
            text("Basic application shell").size(18),
            row![detect_button, test_overlay_button, quit_overlays_button].spacing(12),
            row![text("Selected capture/overlay target:"), monitor_picker]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            text(&self.status),
            scrollable(monitor_rows).height(Length::Fill),
        ]
        .spacing(16)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl Application {
    fn new(context: AppContext) -> Self {
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

        Self {
            context,
            settings,
            monitors: Vec::new(),
            selected_monitor: None,
            overlay_processes: Vec::new(),
            status,
            busy: false,
        }
    }

    fn begin_startup_monitor_detection(&mut self) -> Task<Message> {
        self.busy = true;
        self.status = "Detecting connected monitors...".to_owned();
        Task::perform(
            detect_connected_monitors(),
            Message::StartupMonitorInfoDetected,
        )
    }

    fn begin_monitor_detection(&mut self) {
        self.busy = true;
        self.status = "Polling Wayland output information...".to_owned();
    }

    fn finish_empty_monitor_detection(&mut self) {
        self.busy = false;
        self.status = "Monitor detection completed, but no monitors were returned.".to_owned();
        self.monitors.clear();
        self.selected_monitor = None;
    }

    fn finish_failed_monitor_detection(&mut self, error: impl std::fmt::Display) {
        self.busy = false;
        self.status = format!("Could not query Wayland monitor information: {error}");
    }

    fn finish_monitor_detection(
        &mut self,
        choices: Vec<MonitorChoice>,
        overlay_result: Result<Vec<u32>, String>,
    ) {
        self.busy = false;
        self.selected_monitor = preserve_or_select_first(&self.selected_monitor, &choices);

        let monitor_count = choices.len();
        self.status = match overlay_result {
            Ok(children) => {
                self.overlay_processes.extend(children);
                format!("Detected {monitor_count} monitor(s) and launched info overlays.")
            }
            Err(err) => format!(
                "Detected {monitor_count} monitor(s), but could not launch all info overlays: {err}"
            ),
        };
        self.monitors = choices;
    }

    fn finish_startup_monitor_detection(&mut self, choices: Vec<MonitorChoice>) {
        self.busy = false;
        self.selected_monitor = preserve_or_select_first(&self.selected_monitor, &choices);

        let monitor_count = choices.len();
        self.status = format!("Detected {monitor_count} connected monitor(s).");
        self.monitors = choices;
    }

    fn select_monitor(&mut self, choice: MonitorChoice) {
        self.selected_monitor = Some(choice);
    }

    fn selected_monitor(&self) -> Option<MonitorChoice> {
        self.selected_monitor.clone()
    }

    fn begin_test_overlay_launch(&mut self) -> Option<MonitorChoice> {
        let selected_monitor = self.selected_monitor();
        self.status = selected_monitor
            .as_ref()
            .map(|monitor| format!("Launching test overlay for {}...", monitor.label))
            .unwrap_or_else(|| "Launching test overlay on the active screen...".to_owned());

        selected_monitor
    }

    fn record_test_overlay_launch(&mut self, result: Result<u32, String>) {
        match result {
            Ok(child) => {
                self.overlay_processes.push(child);
                self.status = "Test overlay launched.".to_owned();
            }
            Err(err) => {
                self.status = format!("Could not launch test overlay: {err}");
            }
        }
    }

    fn quit_debug_overlay_processes(&mut self) -> usize {
        let count = self
            .overlay_processes
            .iter()
            .filter(|process_id| {
                ProcessCommand::new("kill")
                    .arg(process_id.to_string())
                    .status()
                    .is_ok()
            })
            .count();

        self.overlay_processes.clear();
        count
    }

    fn report_closed_debug_overlays(&mut self, count: usize) {
        self.status = format!("Closed {count} debug overlay process(es).");
    }

    fn handle_service_event(&mut self, event: ServiceEvent) -> Task<Message> {
        log::debug!("application received watcher service event: {event:?}");

        match event {
            ServiceEvent::LogWatcher(LogWatcherEvent::RewardScreenDetected(detection)) => {
                self.trigger_reward_scan(RewardScanTrigger::Log(detection))
            }
            ServiceEvent::LogWatcher(LogWatcherEvent::Error(failure)) => {
                self.status = format!("Warframe log watcher error: {}", failure.message);
                Task::none()
            }
            ServiceEvent::Hotkey(HotkeyEvent::Pressed(press)) => match press.action {
                HotkeyAction::TriggerRewardScan => {
                    self.trigger_reward_scan(RewardScanTrigger::Hotkey(press.accelerator))
                }
                HotkeyAction::DismissOverlay => {
                    let count = self.quit_debug_overlay_processes();
                    self.status = format!("Dismissed {count} overlay process(es).");
                    Task::none()
                }
                action => {
                    self.status = format!("Hotkey action {action:?} is not implemented yet.");
                    Task::none()
                }
            },
            ServiceEvent::Watcher(event) => {
                self.record_watcher_event(event);
                Task::none()
            }
        }
    }

    fn trigger_reward_scan(&mut self, trigger: RewardScanTrigger) -> Task<Message> {
        log::debug!("triggering reward scan from {trigger:?}");
        self.status = match &trigger {
            RewardScanTrigger::Log(detection) => {
                format!(
                    "Reward screen detected from EE.log marker: {}",
                    detection.marker
                )
            }
            RewardScanTrigger::Hotkey(accelerator) => {
                format!("Reward scan triggered by hotkey {accelerator}.")
            }
        };
        let debug_capture_dir = reward_capture_debug_dir(&self.context);
        let scan_settings = self.reward_scan_settings();

        Task::perform(
            scan_rewards_for_overlay(trigger, scan_settings, debug_capture_dir),
            Message::RewardScanFinished,
        )
    }

    fn reward_scan_settings(&self) -> AppSettings {
        let mut settings = self.settings.clone();

        if let Some(output_name) = self
            .selected_monitor
            .as_ref()
            .and_then(|monitor| monitor.output_name.as_deref())
        {
            log::debug!("using selected monitor {output_name:?} as reward scan capture target");
            settings.set_capture_monitor(output_name);
        } else {
            log::debug!(
                "using configured monitor {:?} as reward scan capture target",
                settings.capture.monitor
            );
        }

        settings
    }

    fn record_reward_scan_finished(
        &mut self,
        result: Result<Vec<RewardOverlayEntry>, String>,
        overlay_result: Option<Result<u32, String>>,
    ) {
        match result {
            Ok(rewards) if rewards.is_empty() => {
                self.status = "Reward scan completed, but no rewards were found.".to_owned();
            }
            Ok(rewards) => match overlay_result {
                Some(Ok(process_id)) => {
                    let reward_count = rewards.len();
                    self.overlay_processes.push(process_id);
                    self.status =
                        format!("Found {reward_count} reward(s) and sent them to the overlay.");
                }
                Some(Err(err)) => {
                    self.status = format!("Found rewards, but could not launch overlay: {err}");
                }
                None => {
                    self.status = format!("Found {} reward(s).", rewards.len());
                }
            },
            Err(err) => {
                self.status = format!("Reward scan failed: {err}");
            }
        }
    }

    fn record_watcher_event(&mut self, event: WatcherEvent) {
        match event {
            WatcherEvent::Started { watcher } => {
                log::debug!("watcher started: {watcher:?}");
            }
            WatcherEvent::Failed { watcher, message } => {
                log::debug!("watcher failed: {watcher:?}: {message}");
                self.status = format!("{} watcher unavailable: {message}", watcher_label(watcher));
            }
            WatcherEvent::Stopped { watcher } => {
                log::debug!("watcher stopped: {watcher:?}");
            }
        }
    }
}

#[derive(Clone, Debug)]
enum Message {
    DetectMonitorInfo,
    StartupMonitorInfoDetected(Result<Vec<monitor::MonitorInfo>, String>),
    MonitorInfoDetected(Result<Vec<monitor::MonitorInfo>, String>),
    SelectedMonitorChanged(MonitorChoice),
    DrawTestOverlay,
    TestOverlayLaunched(Result<u32, String>),
    QuitDebugOverlays,
    ServiceEvent(ServiceEvent),
    RewardScanFinished(Result<Vec<RewardOverlayEntry>, String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MonitorChoice {
    label: String,
    output_name: Option<String>,
    info: monitor::MonitorInfo,
}

impl std::fmt::Display for MonitorChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

async fn detect_and_show_monitor_info() -> Result<Vec<monitor::MonitorInfo>, String> {
    monitor::detect_monitor_info()
}

async fn detect_connected_monitors() -> Result<Vec<monitor::MonitorInfo>, String> {
    monitor::detect_monitor_info()
}

fn monitor_choices(monitors: Vec<monitor::MonitorInfo>) -> Vec<MonitorChoice> {
    monitors
        .into_iter()
        .enumerate()
        .map(|(index, info)| {
            let label = format!("{}: {}", index + 1, info.display_name());
            let output_name = info.layer_shell_output_name().map(str::to_owned);
            MonitorChoice {
                label,
                output_name,
                info,
            }
        })
        .collect()
}

fn preserve_or_select_first(
    current: &Option<MonitorChoice>,
    choices: &[MonitorChoice],
) -> Option<MonitorChoice> {
    current
        .as_ref()
        .and_then(|current| {
            choices
                .iter()
                .find(|choice| choice.output_name == current.output_name)
        })
        .cloned()
        .or_else(|| choices.first().cloned())
}

fn spawn_monitor_info_overlays(monitors: &[MonitorChoice]) -> Result<Vec<u32>, String> {
    let mut children = Vec::new();
    let mut first_error = None;

    for monitor in monitors {
        let mut command = overlay_command()?;
        command.arg("--debug-monitor-info");

        if let Some(output_name) = &monitor.output_name {
            command.arg("--output").arg(output_name);
        }

        for line in monitor.info.summary_lines() {
            command.arg("--line").arg(line);
        }

        match command.spawn() {
            Ok(child) => children.push(child.id()),
            Err(err) => {
                first_error.get_or_insert_with(|| err.to_string());
            }
        }
    }

    first_error.map_or(Ok(children), Err)
}

fn spawn_test_overlay(monitor: Option<&MonitorChoice>) -> Result<u32, String> {
    let mut command = overlay_command()?;
    command.arg("--debug-test-overlay");

    if let Some(output_name) = monitor.and_then(|monitor| monitor.output_name.as_deref()) {
        command.arg("--output").arg(output_name);
    }

    command
        .spawn()
        .map(|child| child.id())
        .map_err(|err| err.to_string())
}

fn spawn_reward_overlay(
    monitor: Option<&MonitorChoice>,
    rewards: &[RewardOverlayEntry],
) -> Result<u32, String> {
    let mut command = overlay_command()?;
    command.arg("--reward-overlay");

    if let Some(output_name) = monitor.and_then(|monitor| monitor.output_name.as_deref()) {
        command.arg("--output").arg(output_name);
    }

    for reward in rewards {
        command.arg("--reward-name").arg(&reward.name);
    }

    command
        .spawn()
        .map(|child| child.id())
        .map_err(|err| err.to_string())
}

fn overlay_command() -> Result<ProcessCommand, String> {
    std::env::current_exe()
        .map(ProcessCommand::new)
        .map_err(|err| err.to_string())
}

fn reward_capture_debug_dir(context: &AppContext) -> PathBuf {
    context
        .config_path()
        .parent()
        .map(|path| path.join("debug").join("reward-captures"))
        .unwrap_or_else(|| PathBuf::from("debug").join("reward-captures"))
}

fn watcher_label(watcher: WatcherKind) -> &'static str {
    match watcher {
        WatcherKind::WarframeLog => "Warframe log",
        WatcherKind::Hotkeys => "Hotkey",
    }
}

#[cfg(test)]
mod tests {
    use super::{Application, monitor_choices};
    use shared::{AppContext, monitor::MonitorInfo, rewards::RewardOverlayEntry};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn monitor_detection_preserves_selected_output_when_available() {
        let mut application = Application::new(test_context());
        let original_choices =
            monitor_choices(vec![monitor_info("DP-1"), monitor_info("HDMI-A-1")]);
        application.finish_monitor_detection(original_choices.clone(), Ok(Vec::new()));
        application.select_monitor(original_choices[1].clone());

        let refreshed_choices =
            monitor_choices(vec![monitor_info("DP-1"), monitor_info("HDMI-A-1")]);
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
}
