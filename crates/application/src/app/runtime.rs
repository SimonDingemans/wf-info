use iced::{Subscription, Task, Theme};
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
            (application, startup_task)
        })
}

impl Application {
    pub(super) fn title(&self) -> String {
        format!("{} debug shell", self.context.name())
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
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

    pub(super) fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            subscriptions::log_watcher::log_watcher_subscription(&self.settings)
                .map(Message::ServiceEvent),
            subscriptions::hotkey_watcher::hotkey_watcher_subscription(&self.settings)
                .map(Message::ServiceEvent),
        ])
    }
}
