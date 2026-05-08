use iced::Task;
use shared::monitor;

use super::message::Message;
use super::state::Application;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MonitorChoice {
    pub(super) label: String,
    pub(super) output_name: Option<String>,
    pub(super) info: monitor::MonitorInfo,
}

impl std::fmt::Display for MonitorChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

impl Application {
    pub(super) fn begin_startup_monitor_detection(&mut self) -> Task<Message> {
        self.busy = true;
        self.status = "Detecting connected monitors...".to_owned();
        Task::perform(
            detect_connected_monitors(),
            Message::StartupMonitorInfoDetected,
        )
    }

    pub(super) fn begin_monitor_detection(&mut self) {
        self.busy = true;
        self.status = "Polling Wayland output information...".to_owned();
    }

    pub(super) fn finish_empty_monitor_detection(&mut self) {
        self.busy = false;
        self.status = "Monitor detection completed, but no monitors were returned.".to_owned();
        self.monitors.clear();
        self.selected_monitor = None;
    }

    pub(super) fn finish_failed_monitor_detection(&mut self, error: impl std::fmt::Display) {
        self.busy = false;
        self.status = format!("Could not query Wayland monitor information: {error}");
    }

    pub(super) fn finish_monitor_detection(
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

    pub(super) fn finish_startup_monitor_detection(&mut self, choices: Vec<MonitorChoice>) {
        self.busy = false;
        self.selected_monitor = preserve_or_select_first(&self.selected_monitor, &choices);

        let monitor_count = choices.len();
        self.status = format!("Detected {monitor_count} connected monitor(s).");
        self.monitors = choices;
    }

    pub(super) fn select_monitor(&mut self, choice: MonitorChoice) {
        self.selected_monitor = Some(choice);
    }

    pub(super) fn selected_monitor(&self) -> Option<MonitorChoice> {
        self.selected_monitor.clone()
    }
}

pub(super) async fn detect_and_show_monitor_info() -> Result<Vec<monitor::MonitorInfo>, String> {
    monitor::detect_monitor_info()
}

async fn detect_connected_monitors() -> Result<Vec<monitor::MonitorInfo>, String> {
    monitor::detect_monitor_info()
}

pub(super) fn monitor_choices(monitors: Vec<monitor::MonitorInfo>) -> Vec<MonitorChoice> {
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
