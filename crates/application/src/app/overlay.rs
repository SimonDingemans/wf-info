use std::process::Command as ProcessCommand;

use shared::rewards::RewardOverlayEntry;

use super::monitor::MonitorChoice;
use super::state::Application;

impl Application {
    pub(super) fn begin_test_overlay_launch(&mut self) -> Option<MonitorChoice> {
        let selected_monitor = self.selected_monitor();
        self.status = selected_monitor
            .as_ref()
            .map(|monitor| format!("Launching test overlay for {}...", monitor.label))
            .unwrap_or_else(|| "Launching test overlay on the active screen...".to_owned());

        selected_monitor
    }

    pub(super) fn record_test_overlay_launch(&mut self, result: Result<u32, String>) {
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

    pub(super) fn quit_debug_overlay_processes(&mut self) -> usize {
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

    pub(super) fn report_closed_debug_overlays(&mut self, count: usize) {
        self.status = format!("Closed {count} debug overlay process(es).");
    }
}

pub(super) fn spawn_monitor_info_overlays(monitors: &[MonitorChoice]) -> Result<Vec<u32>, String> {
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

pub(super) fn spawn_test_overlay(monitor: Option<&MonitorChoice>) -> Result<u32, String> {
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

pub(super) fn spawn_reward_overlay(
    monitor: Option<&MonitorChoice>,
    rewards: &[RewardOverlayEntry],
) -> Result<u32, String> {
    let mut command = overlay_command()?;
    command.arg("--reward-overlay");

    if let Some(output_name) = monitor.and_then(|monitor| monitor.output_name.as_deref()) {
        command.arg("--output").arg(output_name);
    }

    if let Some((width, height)) = monitor.and_then(|monitor| positive_monitor_size(monitor)) {
        command
            .arg("--output-width")
            .arg(width.to_string())
            .arg("--output-height")
            .arg(height.to_string());
    }

    for reward in rewards {
        command.arg("--reward-name").arg(&reward.name);
        append_optional_reward_arg(&mut command, "--reward-platinum", reward.platinum);
        append_optional_reward_arg(&mut command, "--reward-ducats", reward.ducats);
        append_optional_reward_arg(&mut command, "--reward-volume", reward.volume);
        command
            .arg("--reward-vaulted")
            .arg(reward.vaulted.to_string());
    }

    command
        .spawn()
        .map(|child| child.id())
        .map_err(|err| err.to_string())
}

fn append_optional_reward_arg(command: &mut ProcessCommand, name: &str, value: Option<u32>) {
    if let Some(value) = value {
        command.arg(name).arg(value.to_string());
    }
}

fn positive_monitor_size(monitor: &MonitorChoice) -> Option<(u32, u32)> {
    monitor.info.size.and_then(|(width, height)| {
        (width > 0 && height > 0).then_some((width as u32, height as u32))
    })
}

fn overlay_command() -> Result<ProcessCommand, String> {
    std::env::current_exe()
        .map(ProcessCommand::new)
        .map_err(|err| err.to_string())
}
