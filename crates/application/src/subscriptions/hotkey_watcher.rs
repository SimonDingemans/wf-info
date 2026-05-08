use std::sync::mpsc;
use std::thread;

use iced::Subscription;
use iced::futures::channel::mpsc as futures_mpsc;
use iced::futures::{SinkExt, StreamExt};
use iced::stream;
use shared::config::Settings;
use shared::watchers::events::{ServiceEvent, WatcherEvent, WatcherKind};
use shared::watchers::hotkey_watcher::{GlobalHotkeyWatcher, phase_one_hotkey_bindings};

pub fn hotkey_watcher_subscription(settings: &Settings) -> Subscription<ServiceEvent> {
    let bindings = phase_one_hotkey_bindings(&settings.hotkeys);
    let id = format!(
        "hotkey-watcher:{}:{}",
        settings.hotkeys.activation, settings.hotkeys.dismiss_overlay
    );

    Subscription::run_with_id(
        id,
        stream::channel(100, move |mut output| async move {
            let (service_tx, service_rx) = mpsc::channel();
            let (async_tx, mut async_rx) = futures_mpsc::unbounded();
            let bridge = spawn_event_bridge("hotkey", service_rx, async_tx);
            let watcher = match GlobalHotkeyWatcher::spawn(bindings, service_tx.clone()) {
                Ok(handle) => {
                    log::debug!("application hotkey watcher subscription started");
                    let _ = service_tx.send(ServiceEvent::Watcher(WatcherEvent::Started {
                        watcher: WatcherKind::Hotkeys,
                    }));
                    Some(handle)
                }
                Err(err) => {
                    log::debug!("application hotkey watcher subscription failed to start: {err}");
                    let _ = service_tx.send(ServiceEvent::Watcher(WatcherEvent::Failed {
                        watcher: WatcherKind::Hotkeys,
                        message: err.to_string(),
                    }));
                    None
                }
            };
            drop(service_tx);

            while let Some(event) = async_rx.next().await {
                log::debug!("application hotkey watcher subscription forwarding {event:?}");

                if output.send(event).await.is_err() {
                    break;
                }
            }

            if let Some(watcher) = watcher {
                watcher.stop();
            }

            if let Err(err) = bridge.join() {
                log::warn!("application hotkey watcher bridge thread panicked: {err:?}");
            }

            log::debug!("application hotkey watcher subscription stopped");
        }),
    )
}

fn spawn_event_bridge(
    label: &'static str,
    service_rx: mpsc::Receiver<ServiceEvent>,
    async_tx: futures_mpsc::UnboundedSender<ServiceEvent>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name(format!("wf-info-{label}-subscription-bridge"))
        .spawn(move || {
            while let Ok(event) = service_rx.recv() {
                log::debug!("application {label} watcher bridge received {event:?}");

                if async_tx.unbounded_send(event).is_err() {
                    log::debug!("application {label} watcher bridge receiver dropped");
                    break;
                }
            }

            log::debug!("application {label} watcher bridge stopped");
        })
        .expect("subscription bridge thread should spawn")
}
