use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;

use crate::config::Settings;
use crate::watchers::events::ServiceEvent;
use crate::watchers::warframe_log_discovery::{
    WarframeLogDiscoveryError, discover_warframe_log_path,
    discover_warframe_log_path_from_candidates,
};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(250);
const REWARD_SCREEN_MARKERS: &[&str] = &[
    "Pause countdown done",
    "Got rewards",
    "Created /Lotus/Interface/ProjectionRewardChoice.swf",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogWatcherConfig {
    pub path: PathBuf,
    pub poll_interval: Duration,
    pub start_position: LogStartPosition,
    discovery_candidates: Vec<PathBuf>,
}

impl LogWatcherConfig {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            poll_interval: DEFAULT_POLL_INTERVAL,
            start_position: LogStartPosition::End,
            discovery_candidates: Vec::new(),
        }
    }

    pub fn for_settings(settings: &Settings) -> Self {
        Self::new(settings.warframe.log_path.trim())
    }

    pub fn start_at_beginning(mut self) -> Self {
        self.start_position = LogStartPosition::Beginning;
        self
    }

    pub fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    pub fn with_discovery_candidates(mut self, candidates: Vec<PathBuf>) -> Self {
        self.discovery_candidates = candidates;
        self
    }

    fn is_auto_discovery(&self) -> bool {
        self.path.as_os_str().is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogStartPosition {
    Beginning,
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogWatcherEvent {
    LogFileSelected(LogFileSelection),
    RewardScreenDetected(RewardScreenDetection),
    Error(LogWatcherFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFileSelection {
    pub path: PathBuf,
    pub source: LogFileSelectionSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFileSelectionSource {
    Configured,
    Discovered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewardScreenDetection {
    pub path: PathBuf,
    pub marker: String,
    pub line: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogWatcherFailure {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum LogWatcherError {
    #[error("Warframe log path is empty")]
    MissingPath,

    #[error("{message}")]
    Discovery { message: String },

    #[error("failed to open Warframe log {path}: {message}")]
    Open { path: PathBuf, message: String },

    #[error("failed to read Warframe log {path}: {message}")]
    Read { path: PathBuf, message: String },

    #[error("failed to seek Warframe log {path}: {message}")]
    Seek { path: PathBuf, message: String },

    #[error("failed to read Warframe log metadata {path}: {message}")]
    Metadata { path: PathBuf, message: String },
}

impl LogWatcherError {
    fn failure(&self) -> LogWatcherFailure {
        LogWatcherFailure {
            path: self.path().unwrap_or_default(),
            message: self.to_string(),
        }
    }

    fn path(&self) -> Option<PathBuf> {
        match self {
            Self::MissingPath | Self::Discovery { .. } => None,
            Self::Open { path, .. }
            | Self::Read { path, .. }
            | Self::Seek { path, .. }
            | Self::Metadata { path, .. } => Some(path.clone()),
        }
    }
}

pub struct LogWatcherHandle {
    shutdown_tx: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl LogWatcherHandle {
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        let _ = self.shutdown_tx.send(());

        if let Some(thread) = self.thread.take()
            && let Err(err) = thread.join()
        {
            log::warn!("log watcher thread panicked: {err:?}");
        }
    }
}

impl Drop for LogWatcherHandle {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

pub struct LogWatcher;

impl LogWatcher {
    pub fn spawn(
        config: LogWatcherConfig,
        event_tx: Sender<ServiceEvent>,
    ) -> Result<LogWatcherHandle, LogWatcherError> {
        let auto_discovery = config.is_auto_discovery();
        let configured_path = config.path.clone();

        if auto_discovery {
            log::debug!(
                "starting Warframe log watcher with automatic EE.log discovery and {:?} poll interval",
                config.poll_interval
            );
        } else {
            log::debug!(
                "starting Warframe log watcher for {} with {:?} start position and {:?} poll interval",
                config.path.display(),
                config.start_position,
                config.poll_interval
            );
        }

        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let spawn_error_path = config.path.clone();
        let thread = thread::Builder::new()
            .name("wf-info-log-watcher".to_owned())
            .spawn(move || {
                let mut last_reported_failure = None;
                let mut cursor = match open_initial_cursor(&config, &event_tx) {
                    Ok(cursor) => Some(cursor),
                    Err(err) => {
                        log::debug!(
                            "Warframe log watcher could not open configured path {}: {err}",
                            configured_path.display()
                        );
                        report_failure_if_changed(
                            &event_tx,
                            &mut last_reported_failure,
                            err.failure(),
                        );
                        if auto_discovery {
                            None
                        } else {
                            return;
                        }
                    }
                };

                loop {
                    if shutdown_rx.try_recv().is_ok() {
                        log::debug!("Warframe log watcher received shutdown signal");
                        break;
                    }

                    let Some(active_cursor) = cursor.as_mut() else {
                        match open_initial_cursor(&config, &event_tx) {
                            Ok(new_cursor) => {
                                last_reported_failure = None;
                                cursor = Some(new_cursor);
                            }
                            Err(err) => {
                                report_failure_if_changed(
                                    &event_tx,
                                    &mut last_reported_failure,
                                    err.failure(),
                                );
                            }
                        }

                        thread::sleep(config.poll_interval);
                        continue;
                    };

                    match active_cursor.read_new_lines() {
                        Ok(lines) => {
                            if !lines.is_empty() {
                                log::debug!(
                                    "Warframe log watcher read {} new line(s) from {}",
                                    lines.len(),
                                    active_cursor.path.display()
                                );
                            }

                            emit_reward_detections(&event_tx, active_cursor.path.clone(), lines);
                        }
                        Err(err) => {
                            log::debug!("Warframe log watcher read failed: {err}");
                            report_failure_if_changed(
                                &event_tx,
                                &mut last_reported_failure,
                                err.failure(),
                            );

                            if auto_discovery {
                                cursor = None;
                            }
                        }
                    }

                    thread::sleep(config.poll_interval);
                }

                log::debug!("Warframe log watcher stopped");
            })
            .map_err(|err| LogWatcherError::Read {
                path: spawn_error_path,
                message: err.to_string(),
            })?;

        Ok(LogWatcherHandle {
            shutdown_tx,
            thread: Some(thread),
        })
    }
}

fn open_initial_cursor(
    config: &LogWatcherConfig,
    event_tx: &Sender<ServiceEvent>,
) -> Result<LogCursor, LogWatcherError> {
    let selection = resolve_log_file_selection(config)?;
    let cursor = LogCursor::open(&selection.path, config.start_position)?;

    log::debug!(
        "Warframe log watcher opened {} at byte {}",
        selection.path.display(),
        cursor.position
    );
    send_log_watcher_event(event_tx, LogWatcherEvent::LogFileSelected(selection));

    Ok(cursor)
}

fn resolve_log_file_selection(
    config: &LogWatcherConfig,
) -> Result<LogFileSelection, LogWatcherError> {
    if !config.is_auto_discovery() {
        return Ok(LogFileSelection {
            path: config.path.clone(),
            source: LogFileSelectionSource::Configured,
        });
    }

    let discovery = if config.discovery_candidates.is_empty() {
        discover_warframe_log_path()
    } else {
        discover_warframe_log_path_from_candidates(config.discovery_candidates.clone())
    }
    .map_err(discovery_error)?;

    Ok(LogFileSelection {
        path: discovery.path,
        source: LogFileSelectionSource::Discovered,
    })
}

fn discovery_error(err: WarframeLogDiscoveryError) -> LogWatcherError {
    let searched = err.searched().len();

    log::debug!("Warframe EE.log discovery failed after checking {searched} candidate location(s)");

    LogWatcherError::Discovery {
        message: err.user_message(),
    }
}

fn emit_reward_detections(event_tx: &Sender<ServiceEvent>, path: PathBuf, lines: Vec<String>) {
    for line in lines {
        if let Some(marker) = reward_screen_marker(&line) {
            log::debug!("Warframe log watcher detected reward marker {marker:?}");
            let detection = RewardScreenDetection {
                path: path.clone(),
                marker: marker.to_owned(),
                line,
            };
            send_log_watcher_event(event_tx, LogWatcherEvent::RewardScreenDetected(detection));
        }
    }
}

fn report_failure_if_changed(
    event_tx: &Sender<ServiceEvent>,
    last_reported_failure: &mut Option<LogWatcherFailure>,
    failure: LogWatcherFailure,
) {
    if last_reported_failure.as_ref() == Some(&failure) {
        return;
    }

    *last_reported_failure = Some(failure.clone());
    send_log_watcher_event(event_tx, LogWatcherEvent::Error(failure));
}

pub fn reward_screen_marker(line: &str) -> Option<&'static str> {
    REWARD_SCREEN_MARKERS
        .iter()
        .copied()
        .find(|marker| line.contains(marker))
}

fn send_log_watcher_event(event_tx: &Sender<ServiceEvent>, event: LogWatcherEvent) {
    log::debug!("Warframe log watcher emitting event: {event:?}");

    if event_tx.send(ServiceEvent::LogWatcher(event)).is_err() {
        log::debug!("log watcher receiver dropped");
    }
}

struct LogCursor {
    path: PathBuf,
    position: u64,
}

impl LogCursor {
    fn open(path: &Path, start_position: LogStartPosition) -> Result<Self, LogWatcherError> {
        let mut file = open_file(path)?;
        let position = match start_position {
            LogStartPosition::Beginning => 0,
            LogStartPosition::End => {
                file.seek(SeekFrom::End(0))
                    .map_err(|err| LogWatcherError::Seek {
                        path: path.to_path_buf(),
                        message: err.to_string(),
                    })?
            }
        };

        Ok(Self {
            path: path.to_path_buf(),
            position,
        })
    }

    fn read_new_lines(&mut self) -> Result<Vec<String>, LogWatcherError> {
        let mut file = open_file(&self.path)?;
        let length = file
            .metadata()
            .map_err(|err| LogWatcherError::Metadata {
                path: self.path.clone(),
                message: err.to_string(),
            })?
            .len();

        if length < self.position {
            self.position = 0;
        }

        file.seek(SeekFrom::Start(self.position))
            .map_err(|err| LogWatcherError::Seek {
                path: self.path.clone(),
                message: err.to_string(),
            })?;

        let mut reader = BufReader::new(file);
        let mut lines = Vec::new();

        loop {
            let mut line = String::new();
            let bytes = reader
                .read_line(&mut line)
                .map_err(|err| LogWatcherError::Read {
                    path: self.path.clone(),
                    message: err.to_string(),
                })?;

            if bytes == 0 {
                break;
            }

            lines.push(line.trim_end_matches(['\r', '\n']).to_owned());
        }

        self.position = reader
            .stream_position()
            .map_err(|err| LogWatcherError::Seek {
                path: self.path.clone(),
                message: err.to_string(),
            })?;

        Ok(lines)
    }
}

fn open_file(path: &Path) -> Result<File, LogWatcherError> {
    File::open(path).map_err(|err| LogWatcherError::Open {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{
        LogCursor, LogFileSelectionSource, LogStartPosition, LogWatcher, LogWatcherConfig,
        LogWatcherEvent, reward_screen_marker,
    };
    use crate::config::Settings;
    use crate::watchers::events::ServiceEvent;

    #[test]
    fn reward_screen_marker_matches_known_warframe_log_lines() {
        assert_eq!(
            reward_screen_marker("Script [Info]: Pause countdown done"),
            Some("Pause countdown done")
        );
        assert_eq!(
            reward_screen_marker("Game [Info]: Got rewards after fissure"),
            Some("Got rewards")
        );
        assert_eq!(
            reward_screen_marker("Created /Lotus/Interface/ProjectionRewardChoice.swf"),
            Some("Created /Lotus/Interface/ProjectionRewardChoice.swf")
        );
        assert_eq!(reward_screen_marker("unrelated log line"), None);
    }

    #[test]
    fn config_uses_warframe_log_path_from_settings() {
        let mut settings = Settings::default();
        settings.warframe.log_path = "/tmp/EE.log".to_owned();

        let config = LogWatcherConfig::for_settings(&settings);

        assert_eq!(config.path, PathBuf::from("/tmp/EE.log"));
        assert_eq!(config.start_position, LogStartPosition::End);
        assert_eq!(config.poll_interval, Duration::from_millis(250));
    }

    #[test]
    fn blank_config_path_enables_auto_discovery() {
        let settings = Settings::default();

        let config = LogWatcherConfig::for_settings(&settings);

        assert!(config.path.as_os_str().is_empty());
        assert!(config.is_auto_discovery());
    }

    #[test]
    fn cursor_reads_only_lines_added_after_starting_at_end() {
        let path = temp_log_path("start-at-end");
        fs::write(&path, "old line\n").expect("fixture should write");
        let mut cursor = LogCursor::open(&path, LogStartPosition::End).expect("cursor should open");

        fs::write(&path, "old line\nnew line\n").expect("fixture should append content");

        assert_eq!(
            cursor
                .read_new_lines()
                .expect("new lines should be readable"),
            vec!["new line"]
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn cursor_resets_when_log_file_is_truncated() {
        let path = temp_log_path("truncated");
        fs::write(&path, "old line that is longer\n").expect("fixture should write");
        let mut cursor = LogCursor::open(&path, LogStartPosition::End).expect("cursor should open");

        fs::write(&path, "new line\n").expect("fixture should truncate and rewrite");

        assert_eq!(
            cursor
                .read_new_lines()
                .expect("truncated log should be readable"),
            vec!["new line"]
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn spawned_watcher_emits_reward_screen_detection_events() {
        let path = temp_log_path("spawned");
        fs::write(
            &path,
            "Created /Lotus/Interface/ProjectionRewardChoice.swf\n",
        )
        .expect("fixture should write");
        let (event_tx, event_rx) = mpsc::channel();
        let config = LogWatcherConfig::new(&path)
            .start_at_beginning()
            .with_poll_interval(Duration::from_millis(10));

        let handle = LogWatcher::spawn(config, event_tx).expect("watcher should spawn");
        let events = receive_events(&event_rx, 2);
        handle.stop();

        assert!(events.iter().any(|event| matches!(
            event,
            ServiceEvent::LogWatcher(LogWatcherEvent::RewardScreenDetected(detection))
                if detection.marker == "Created /Lotus/Interface/ProjectionRewardChoice.swf"
        )));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn spawned_watcher_emits_configured_log_file_selection() {
        let path = temp_log_path("configured-selection");
        fs::write(&path, "old line\n").expect("fixture should write");
        let (event_tx, event_rx) = mpsc::channel();
        let config = LogWatcherConfig::new(&path).with_poll_interval(Duration::from_millis(10));

        let handle = LogWatcher::spawn(config, event_tx).expect("watcher should spawn");
        let event = event_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("watcher should emit a selection event");
        handle.stop();

        assert!(matches!(
            event,
            ServiceEvent::LogWatcher(LogWatcherEvent::LogFileSelected(selection))
                if selection.path == path && selection.source == LogFileSelectionSource::Configured
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn spawned_watcher_auto_discovers_log_file() {
        let path = temp_log_path("auto-discovered");
        fs::write(
            &path,
            "Created /Lotus/Interface/ProjectionRewardChoice.swf\n",
        )
        .expect("fixture should write");
        let (event_tx, event_rx) = mpsc::channel();
        let config = LogWatcherConfig::new("")
            .start_at_beginning()
            .with_poll_interval(Duration::from_millis(10))
            .with_discovery_candidates(vec![path.clone()]);

        let handle = LogWatcher::spawn(config, event_tx).expect("watcher should spawn");
        let events = receive_events(&event_rx, 2);
        handle.stop();

        assert!(events.iter().any(|event| matches!(
            event,
            ServiceEvent::LogWatcher(LogWatcherEvent::LogFileSelected(selection))
                if selection.path == path && selection.source == LogFileSelectionSource::Discovered
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ServiceEvent::LogWatcher(LogWatcherEvent::RewardScreenDetected(detection))
                if detection.path == path
        )));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn spawned_watcher_retries_auto_discovery_until_log_exists() {
        let path = temp_log_path("delayed-discovery");
        let _ = fs::remove_file(&path);
        let (event_tx, event_rx) = mpsc::channel();
        let config = LogWatcherConfig::new("")
            .start_at_beginning()
            .with_poll_interval(Duration::from_millis(10))
            .with_discovery_candidates(vec![path.clone()]);

        let handle = LogWatcher::spawn(config, event_tx).expect("watcher should spawn");
        let first_event = event_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("watcher should report initial discovery failure");
        fs::write(&path, "Got rewards\n").expect("fixture should write");
        let mut events = vec![first_event];
        events.extend(receive_events(&event_rx, 2));
        handle.stop();

        assert!(events.iter().any(|event| matches!(
            event,
            ServiceEvent::LogWatcher(LogWatcherEvent::Error(failure))
                if failure.message.contains("Could not discover")
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ServiceEvent::LogWatcher(LogWatcherEvent::LogFileSelected(selection))
                if selection.path == path
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ServiceEvent::LogWatcher(LogWatcherEvent::RewardScreenDetected(detection))
                if detection.marker == "Got rewards"
        )));

        let _ = fs::remove_file(path);
    }

    fn receive_events(
        event_rx: &mpsc::Receiver<ServiceEvent>,
        expected_count: usize,
    ) -> Vec<ServiceEvent> {
        let mut events = Vec::new();

        while events.len() < expected_count {
            events.push(
                event_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("watcher should emit an event"),
            );
        }

        events
    }

    fn temp_log_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("wf-info-{name}-{suffix}.log"))
    }
}
