use std::path::PathBuf;

use iced::Task;
use shared::AppContext;
use shared::config::Settings as AppSettings;
use shared::rewards::RewardOverlayEntry;

use crate::reward_scan::{RewardScanTrigger, scan_rewards_for_overlay};

use super::message::Message;
use super::state::Application;

impl Application {
    pub(super) fn trigger_reward_scan(&mut self, trigger: RewardScanTrigger) -> Task<Message> {
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

    pub(super) fn reward_scan_settings(&self) -> AppSettings {
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

    pub(super) fn record_reward_scan_finished(
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
}

fn reward_capture_debug_dir(context: &AppContext) -> PathBuf {
    context
        .config_path()
        .parent()
        .map(|path| path.join("debug").join("reward-captures"))
        .unwrap_or_else(|| PathBuf::from("debug").join("reward-captures"))
}
