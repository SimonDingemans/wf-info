use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;

use crate::config::Settings;
use crate::watchers::events::ServiceEvent;

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
}

impl LogWatcherConfig {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            poll_interval: DEFAULT_POLL_INTERVAL,
            start_position: LogStartPosition::End,
        }
    }

    pub fn for_settings(settings: &Settings) -> Self {
        Self::new(settings.warframe.log_path.clone())
    }

    pub fn start_at_beginning(mut self) -> Self {
        self.start_position = LogStartPosition::Beginning;
        self
    }

    pub fn with_poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogStartPosition {
    Beginning,
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogWatcherEvent {
    RewardScreenDetected(RewardScreenDetection),
    Error(LogWatcherFailure),
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
            Self::MissingPath => None,
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
        log::debug!(
            "starting Warframe log watcher for {} with {:?} start position and {:?} poll interval",
            config.path.display(),
            config.start_position,
            config.poll_interval
        );

        if config.path.as_os_str().is_empty() {
            log::debug!(
                "Warframe log watcher was not started because the configured path is empty"
            );
            return Err(LogWatcherError::MissingPath);
        }

        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let spawn_error_path = config.path.clone();
        let thread = thread::Builder::new()
            .name("wf-info-log-watcher".to_owned())
            .spawn(move || {
                let mut cursor = match LogCursor::open(&config.path, config.start_position) {
                    Ok(cursor) => cursor,
                    Err(err) => {
                        log::debug!(
                            "Warframe log watcher could not open {}: {err}",
                            config.path.display()
                        );
                        send_log_watcher_event(&event_tx, LogWatcherEvent::Error(err.failure()));
                        return;
                    }
                };

                log::debug!(
                    "Warframe log watcher opened {} at byte {}",
                    config.path.display(),
                    cursor.position
                );

                loop {
                    if shutdown_rx.try_recv().is_ok() {
                        log::debug!("Warframe log watcher received shutdown signal");
                        break;
                    }

                    match cursor.read_new_lines() {
                        Ok(lines) => {
                            if !lines.is_empty() {
                                log::debug!(
                                    "Warframe log watcher read {} new line(s) from {}",
                                    lines.len(),
                                    config.path.display()
                                );
                            }

                            for line in lines {
                                if let Some(marker) = reward_screen_marker(&line) {
                                    log::debug!(
                                        "Warframe log watcher detected reward marker {marker:?}"
                                    );
                                    let detection = RewardScreenDetection {
                                        path: config.path.clone(),
                                        marker: marker.to_owned(),
                                        line,
                                    };
                                    send_log_watcher_event(
                                        &event_tx,
                                        LogWatcherEvent::RewardScreenDetected(detection),
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            log::debug!("Warframe log watcher read failed: {err}");
                            send_log_watcher_event(
                                &event_tx,
                                LogWatcherEvent::Error(err.failure()),
                            );
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
        LogCursor, LogStartPosition, LogWatcher, LogWatcherConfig, LogWatcherEvent,
        reward_screen_marker,
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
        let event = event_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("watcher should emit a detection event");
        handle.stop();

        assert!(matches!(
            event,
            ServiceEvent::LogWatcher(LogWatcherEvent::RewardScreenDetected(detection))
                if detection.marker == "Created /Lotus/Interface/ProjectionRewardChoice.swf"
        ));

        let _ = fs::remove_file(path);
    }

    fn temp_log_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("wf-info-{name}-{suffix}.log"))
    }
}
