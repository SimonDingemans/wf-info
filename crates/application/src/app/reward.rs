use std::path::PathBuf;

use iced::Task;
use shared::AppContext;
use shared::config::Settings as AppSettings;
use shared::monitor::{self, MonitorCaptureRegion};
use shared::rewards::RewardOverlayEntry;

use crate::reward_scan::{RewardScanTrigger, scan_rewards_for_overlay};

use super::message::Message;
use super::state::Application;

impl Application {
    pub(super) fn trigger_manual_reward_scan(&mut self) -> Task<Message> {
        if !self.settings.scanner.enabled {
            self.status =
                "Reward scanner is disabled. Enable it in settings to scan now.".to_owned();
            return Task::none();
        }

        self.trigger_reward_scan(RewardScanTrigger::Manual)
    }

    pub(super) fn trigger_reward_scan(&mut self, trigger: RewardScanTrigger) -> Task<Message> {
        if self.reward_scan_in_progress {
            log::debug!("ignoring reward scan trigger {trigger:?}; scan already in progress");
            self.status = "Reward scan is already running.".to_owned();
            return Task::none();
        }

        log::debug!("triggering reward scan from {trigger:?}");
        self.reward_scan_in_progress = true;
        self.status = match &trigger {
            RewardScanTrigger::Manual => "Reward scan started.".to_owned(),
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
        let item_database_cache_dir = self.context.cache_dir().to_path_buf();
        let scan_settings = self.reward_scan_settings();
        let monitor_region = self.reward_scan_monitor_region();

        Task::perform(
            scan_rewards_for_overlay(
                trigger,
                scan_settings,
                debug_capture_dir,
                item_database_cache_dir,
                monitor_region,
            ),
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

    pub(super) fn reward_scan_monitor_region(&self) -> Option<MonitorCaptureRegion> {
        let selected_monitor = self.selected_monitor.as_ref()?;
        let monitors = self
            .monitors
            .iter()
            .map(|choice| choice.info.clone())
            .collect::<Vec<_>>();

        match monitor::capture_region_for_monitor(&monitors, &selected_monitor.info) {
            Ok(region) => Some(region),
            Err(err) => {
                log::warn!("could not reuse selected monitor geometry for reward scan: {err}");
                None
            }
        }
    }

    pub(super) fn record_reward_scan_finished(
        &mut self,
        result: Result<Vec<RewardOverlayEntry>, String>,
        overlay_result: Option<Result<u32, String>>,
        clipboard_queued: bool,
    ) {
        self.reward_scan_in_progress = false;
        self.last_reward_scan = Some(result.clone());

        match result {
            Ok(rewards) if rewards.is_empty() => {
                self.status = "Reward scan completed, but no rewards were found.".to_owned();
            }
            Ok(rewards) => match overlay_result {
                Some(Ok(process_id)) => {
                    let reward_count = rewards.len();
                    self.overlay_processes.push(process_id);
                    self.status = reward_scan_success_status(
                        format!("Found {reward_count} reward(s) and sent them to the overlay."),
                        clipboard_queued,
                    );
                }
                Some(Err(err)) => {
                    self.status = reward_scan_success_status(
                        format!("Found rewards, but could not launch overlay: {err}"),
                        clipboard_queued,
                    );
                }
                None => {
                    self.status = reward_scan_success_status(
                        format!("Found {} reward(s).", rewards.len()),
                        clipboard_queued,
                    );
                }
            },
            Err(err) => {
                self.status = format!("Reward scan failed: {err}");
            }
        }
    }
}

fn reward_scan_success_status(base: String, clipboard_queued: bool) -> String {
    if clipboard_queued {
        format!("{base} Clipboard summary queued.")
    } else {
        base
    }
}

fn reward_capture_debug_dir(context: &AppContext) -> PathBuf {
    context
        .config_path()
        .parent()
        .map(|path| path.join("debug").join("reward-captures"))
        .unwrap_or_else(|| PathBuf::from("debug").join("reward-captures"))
}
