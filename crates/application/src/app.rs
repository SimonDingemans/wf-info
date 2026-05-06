use std::process::Command as ProcessCommand;

use iced::widget::{button, column, container, pick_list, row, scrollable, text};
use iced::{Element, Length, Task, Theme};
use shared::{AppContext, monitor};

pub fn run(context: &AppContext) -> iced::Result {
    let context = context.clone();

    iced::application(title, update, view)
        .theme(|_| Theme::Dark)
        .run_with(move || (Application::new(context), Task::none()))
}

#[derive(Debug)]
struct Application {
    context: AppContext,
    monitors: Vec<MonitorChoice>,
    selected_monitor: Option<MonitorChoice>,
    overlay_processes: Vec<u32>,
    status: String,
    busy: bool,
}

impl Application {
    fn new(context: AppContext) -> Self {
        Self {
            context,
            monitors: Vec::new(),
            selected_monitor: None,
            overlay_processes: Vec::new(),
            status: "Ready. Use the debug buttons to query monitor streams or draw overlays."
                .to_owned(),
            busy: false,
        }
    }

    fn begin_monitor_detection(&mut self) {
        self.busy = true;
        self.status = "Polling Wayland output information...".to_owned();
    }

    fn finish_empty_monitor_detection(&mut self) {
        self.busy = false;
        self.status = "The portal completed, but it did not return any monitor streams.".to_owned();
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
}

#[derive(Clone, Debug)]
enum Message {
    DetectMonitorInfo,
    MonitorInfoDetected(Result<Vec<monitor::MonitorInfo>, String>),
    SelectedMonitorChanged(MonitorChoice),
    DrawTestOverlay,
    TestOverlayLaunched(Result<u32, String>),
    QuitDebugOverlays,
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

fn title(application: &Application) -> String {
    format!("{} debug shell", application.context.name())
}

fn update(application: &mut Application, message: Message) -> Task<Message> {
    match message {
        Message::DetectMonitorInfo => {
            application.begin_monitor_detection();
            Task::perform(detect_and_show_monitor_info(), Message::MonitorInfoDetected)
        }
        Message::MonitorInfoDetected(Ok(monitors)) => {
            if monitors.is_empty() {
                application.finish_empty_monitor_detection();
                return Task::none();
            }

            let choices = monitor_choices(monitors);
            let overlay_result = spawn_monitor_info_overlays(&choices);
            application.finish_monitor_detection(choices, overlay_result);
            Task::none()
        }
        Message::MonitorInfoDetected(Err(err)) => {
            application.finish_failed_monitor_detection(err);
            Task::none()
        }
        Message::SelectedMonitorChanged(choice) => {
            application.select_monitor(choice);
            Task::none()
        }
        Message::DrawTestOverlay => {
            let selected_monitor = application.begin_test_overlay_launch();

            Task::perform(
                async move { spawn_test_overlay(selected_monitor.as_ref()) },
                Message::TestOverlayLaunched,
            )
        }
        Message::TestOverlayLaunched(result) => {
            application.record_test_overlay_launch(result);
            Task::none()
        }
        Message::QuitDebugOverlays => {
            let count = application.quit_debug_overlay_processes();
            application.report_closed_debug_overlays(count);
            Task::none()
        }
    }
}

fn view(application: &Application) -> Element<'_, Message> {
    let detect_button = button("Show Monitor Info Overlays")
        .padding([10, 14])
        .on_press_maybe((!application.busy).then_some(Message::DetectMonitorInfo));

    let test_overlay_button = button("Draw Test Overlay")
        .padding([10, 14])
        .on_press(Message::DrawTestOverlay);
    let quit_overlays_button = button("Quit Debug Overlays")
        .padding([10, 14])
        .on_press(Message::QuitDebugOverlays);

    let selected = application.selected_monitor.clone();
    let monitor_picker = pick_list(
        application.monitors.as_slice(),
        selected,
        Message::SelectedMonitorChanged,
    )
    .placeholder("Active screen");

    let monitor_rows = application
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
        row![text("Selected overlay target:"), monitor_picker]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        text(&application.status),
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

async fn detect_and_show_monitor_info() -> Result<Vec<monitor::MonitorInfo>, String> {
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

fn overlay_command() -> Result<ProcessCommand, String> {
    std::env::current_exe()
        .map(ProcessCommand::new)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{Application, monitor_choices};
    use shared::{AppContext, monitor::MonitorInfo};

    #[test]
    fn monitor_detection_preserves_selected_output_when_available() {
        let mut application = Application::new(AppContext::new("wf-info-test"));
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
        let mut application = Application::new(AppContext::new("wf-info-test"));
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
        let mut application = Application::new(AppContext::new("wf-info-test"));

        application.record_test_overlay_launch(Ok(99));

        assert_eq!(application.overlay_processes, vec![99]);
        assert_eq!(application.status, "Test overlay launched.");

        application.record_test_overlay_launch(Err("boom".to_owned()));

        assert_eq!(application.overlay_processes, vec![99]);
        assert_eq!(application.status, "Could not launch test overlay: boom");
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
