use iced::Task;
use shared::watchers::events::{ServiceEvent, WatcherEvent, WatcherKind};
use shared::watchers::hotkey_watcher::{HotkeyAction, HotkeyEvent};
use shared::watchers::log_watcher::{LogFileSelectionSource, LogWatcherEvent};

use crate::reward_scan::RewardScanTrigger;

use super::message::Message;
use super::state::Application;

impl Application {
    pub(super) fn handle_service_event(&mut self, event: ServiceEvent) -> Task<Message> {
        log::debug!("application received watcher service event: {event:?}");

        match event {
            ServiceEvent::LogWatcher(LogWatcherEvent::LogFileSelected(selection)) => {
                self.status = match selection.source {
                    LogFileSelectionSource::Configured => {
                        format!("Watching Warframe log: {}", selection.path.display())
                    }
                    LogFileSelectionSource::Discovered => {
                        format!("Discovered Warframe log: {}", selection.path.display())
                    }
                };
                Task::none()
            }
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

fn watcher_label(watcher: WatcherKind) -> &'static str {
    match watcher {
        WatcherKind::WarframeLog => "Warframe log",
        WatcherKind::Hotkeys => "Hotkey",
    }
}
