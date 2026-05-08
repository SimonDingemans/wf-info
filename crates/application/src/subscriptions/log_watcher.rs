use std::time::Duration;

use iced::Subscription;

use shared::config::Settings;
use shared::watchers::events::{ServiceEvent, WatcherEvent, WatcherKind};
use shared::watchers::log_watcher::{
    LogWatcher, LogWatcherConfig, LogWatcherEvent, LogWatcherFailure,
};

use super::watcher_subscription;

pub fn log_watcher_subscription(settings: &Settings) -> Subscription<ServiceEvent> {
    if !settings.scanner.enabled {
        log::debug!("log watcher subscription disabled because scanner.enabled=false");
        return Subscription::none();
    }

    let config = LogWatcherConfig::for_settings(settings);
    let capture_delay = Duration::from_millis(settings.scanner.auto_delay_ms);
    let log_source = if config.path.as_os_str().is_empty() {
        "auto-discovery".to_owned()
    } else {
        config.path.display().to_string()
    };
    let id = format!(
        "log-watcher:{}:{}",
        log_source, settings.scanner.auto_delay_ms
    );

    watcher_subscription(
        id,
        "log",
        move |service_tx| {
            let error_path = config.path.clone();
            match LogWatcher::spawn(config, service_tx.clone()) {
                Ok(handle) => {
                    log::debug!("application log watcher subscription started");
                    let _ = service_tx.send(ServiceEvent::Watcher(WatcherEvent::Started {
                        watcher: WatcherKind::WarframeLog,
                    }));
                    Some(handle)
                }
                Err(err) => {
                    log::debug!("application log watcher subscription failed to start: {err}");
                    let _ = service_tx.send(ServiceEvent::Watcher(WatcherEvent::Failed {
                        watcher: WatcherKind::WarframeLog,
                        message: err.to_string(),
                    }));
                    let _ = service_tx.send(ServiceEvent::LogWatcher(LogWatcherEvent::Error(
                        LogWatcherFailure {
                            path: error_path,
                            message: err.to_string(),
                        },
                    )));
                    None
                }
            }
        },
        move |event| delay_reward_detection_event(event, capture_delay),
        |watcher| watcher.stop(),
    )
}

fn delay_reward_detection_event(event: &ServiceEvent, capture_delay: Duration) {
    if matches!(
        event,
        ServiceEvent::LogWatcher(LogWatcherEvent::RewardScreenDetected(_))
    ) && !capture_delay.is_zero()
    {
        log::debug!(
            "application log watcher bridge delaying reward scan event by {:?}",
            capture_delay
        );
        std::thread::sleep(capture_delay);
    }
}
