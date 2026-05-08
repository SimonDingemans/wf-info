use iced::Subscription;
use shared::config::Settings;
use shared::watchers::events::{ServiceEvent, WatcherEvent, WatcherKind};
use shared::watchers::hotkey_watcher::{GlobalHotkeyWatcher, phase_one_hotkey_bindings};

use super::watcher_subscription;

pub fn hotkey_watcher_subscription(settings: &Settings) -> Subscription<ServiceEvent> {
    let bindings = phase_one_hotkey_bindings(&settings.hotkeys);
    let id = format!(
        "hotkey-watcher:{}:{}",
        settings.hotkeys.activation, settings.hotkeys.dismiss_overlay
    );

    watcher_subscription(
        id,
        "hotkey",
        move |service_tx| match GlobalHotkeyWatcher::spawn(bindings, service_tx.clone()) {
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
        },
        |_| {},
        |watcher| watcher.stop(),
    )
}
