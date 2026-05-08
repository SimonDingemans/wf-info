use crate::watchers::hotkey_watcher::HotkeyEvent;
use crate::watchers::log_watcher::LogWatcherEvent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceEvent {
    LogWatcher(LogWatcherEvent),
    Hotkey(HotkeyEvent),
    Watcher(WatcherEvent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatcherEvent {
    Started {
        watcher: WatcherKind,
    },
    Failed {
        watcher: WatcherKind,
        message: String,
    },
    Stopped {
        watcher: WatcherKind,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatcherKind {
    WarframeLog,
    Hotkeys,
}
