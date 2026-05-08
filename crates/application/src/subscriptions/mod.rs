use std::sync::mpsc::{self, Sender};
use std::thread;

use iced::Subscription;
use iced::futures::channel::mpsc as futures_mpsc;
use iced::futures::{SinkExt, StreamExt};
use iced::stream;
use shared::watchers::events::ServiceEvent;

pub mod hotkey_watcher;
pub mod log_watcher;

pub(super) fn watcher_subscription<Handle>(
    id: String,
    label: &'static str,
    start_watcher: impl FnOnce(Sender<ServiceEvent>) -> Option<Handle> + Send + 'static,
    before_forward: impl Fn(&ServiceEvent) + Send + 'static,
    stop_watcher: impl FnOnce(Handle) + Send + 'static,
) -> Subscription<ServiceEvent>
where
    Handle: Send + 'static,
{
    Subscription::run_with_id(
        id,
        stream::channel(100, move |mut output| async move {
            let (service_tx, service_rx) = mpsc::channel();
            let (async_tx, mut async_rx) = futures_mpsc::unbounded();
            let bridge = spawn_event_bridge(label, service_rx, async_tx, before_forward);
            let watcher = start_watcher(service_tx.clone());
            drop(service_tx);

            while let Some(event) = async_rx.next().await {
                log::debug!("application {label} watcher subscription forwarding {event:?}");

                if output.send(event).await.is_err() {
                    break;
                }
            }

            if let Some(watcher) = watcher {
                stop_watcher(watcher);
            }

            if let Err(err) = bridge.join() {
                log::warn!("application {label} watcher bridge thread panicked: {err:?}");
            }

            log::debug!("application {label} watcher subscription stopped");
        }),
    )
}

fn spawn_event_bridge(
    label: &'static str,
    service_rx: mpsc::Receiver<ServiceEvent>,
    async_tx: futures_mpsc::UnboundedSender<ServiceEvent>,
    before_forward: impl Fn(&ServiceEvent) + Send + 'static,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name(format!("wf-info-{label}-subscription-bridge"))
        .spawn(move || {
            while let Ok(event) = service_rx.recv() {
                log::debug!("application {label} watcher bridge received {event:?}");
                before_forward(&event);

                if async_tx.unbounded_send(event).is_err() {
                    log::debug!("application {label} watcher bridge receiver dropped");
                    break;
                }
            }

            log::debug!("application {label} watcher bridge stopped");
        })
        .expect("subscription bridge thread should spawn")
}
