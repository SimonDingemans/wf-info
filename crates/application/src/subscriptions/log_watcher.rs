use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use iced::Subscription;
use iced::futures::channel::mpsc as futures_mpsc;
use iced::futures::{SinkExt, StreamExt};
use iced::stream;

use shared::config::Settings;
use shared::watchers::events::{ServiceEvent, WatcherEvent, WatcherKind};
use shared::watchers::log_watcher::{
    LogWatcher, LogWatcherConfig, LogWatcherEvent, LogWatcherFailure,
};

pub fn log_watcher_subscription(settings: &Settings) -> Subscription<ServiceEvent> {
    if !settings.scanner.enabled {
        log::debug!("log watcher subscription disabled because scanner.enabled=false");
        return Subscription::none();
    }

    if settings.warframe.log_path.trim().is_empty() {
        log::debug!("log watcher subscription disabled because warframe.log_path is empty");
        return Subscription::none();
    }

    let config = LogWatcherConfig::for_settings(settings);
    let capture_delay = Duration::from_millis(settings.scanner.auto_delay_ms);
    let id = format!(
        "log-watcher:{}:{}",
        config.path.display(),
        settings.scanner.auto_delay_ms
    );

    Subscription::run_with_id(
        id,
        stream::channel(100, move |mut output| async move {
            let (service_tx, service_rx) = mpsc::channel();
            let (async_tx, mut async_rx) = futures_mpsc::unbounded();
            let bridge = spawn_event_bridge("log", service_rx, async_tx, capture_delay);
            let error_path = config.path.clone();
            let watcher = match LogWatcher::spawn(config, service_tx.clone()) {
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
            };
            drop(service_tx);

            while let Some(event) = async_rx.next().await {
                log::debug!("application log watcher subscription forwarding {event:?}");

                if output.send(event).await.is_err() {
                    break;
                }
            }

            if let Some(watcher) = watcher {
                watcher.stop();
            }

            if let Err(err) = bridge.join() {
                log::warn!("application log watcher bridge thread panicked: {err:?}");
            }

            log::debug!("application log watcher subscription stopped");
        }),
    )
}

fn spawn_event_bridge(
    label: &'static str,
    service_rx: mpsc::Receiver<ServiceEvent>,
    async_tx: futures_mpsc::UnboundedSender<ServiceEvent>,
    capture_delay: Duration,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name(format!("wf-info-{label}-subscription-bridge"))
        .spawn(move || {
            while let Ok(event) = service_rx.recv() {
                log::debug!("application {label} watcher bridge received {event:?}");

                if matches!(
                    event,
                    ServiceEvent::LogWatcher(LogWatcherEvent::RewardScreenDetected(_))
                ) && !capture_delay.is_zero()
                {
                    log::debug!(
                        "application {label} watcher bridge delaying reward scan event by {:?}",
                        capture_delay
                    );
                    thread::sleep(capture_delay);
                }

                if async_tx.unbounded_send(event).is_err() {
                    log::debug!("application {label} watcher bridge receiver dropped");
                    break;
                }
            }

            log::debug!("application {label} watcher bridge stopped");
        })
        .expect("subscription bridge thread should spawn")
}
