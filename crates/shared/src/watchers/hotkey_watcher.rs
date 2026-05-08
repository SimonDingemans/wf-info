use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ashpd::desktop::CreateSessionOptions;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use futures_util::{FutureExt, StreamExt, pin_mut, select};
use thiserror::Error;

use crate::config::HotkeyConfig;
use crate::watchers::events::ServiceEvent;

const HOTKEY_SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    TriggerRewardScan,
    DismissOverlay,
    DebugScreenshot,
    SnapIt,
    SearchIt,
    MasterIt,
}

impl HotkeyAction {
    fn portal_id(&self) -> &'static str {
        match self {
            Self::TriggerRewardScan => "trigger-reward-scan",
            Self::DismissOverlay => "dismiss-overlay",
            Self::DebugScreenshot => "debug-screenshot",
            Self::SnapIt => "snap-it",
            Self::SearchIt => "search-it",
            Self::MasterIt => "master-it",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::TriggerRewardScan => "Trigger reward screen scan",
            Self::DismissOverlay => "Dismiss reward overlay",
            Self::DebugScreenshot => "Load debug screenshot",
            Self::SnapIt => "Run Snap It",
            Self::SearchIt => "Run Search It",
            Self::MasterIt => "Run Master It",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub action: HotkeyAction,
    pub accelerator: String,
}

impl HotkeyBinding {
    pub fn new(action: HotkeyAction, accelerator: impl Into<String>) -> Self {
        Self {
            action,
            accelerator: accelerator.into(),
        }
    }

    fn portal_shortcut(&self) -> Result<NewShortcut, HotkeyWatcherError> {
        let preferred_trigger = self.accelerator.trim();

        if preferred_trigger.is_empty() {
            return Err(HotkeyWatcherError::InvalidAccelerator {
                accelerator: self.accelerator.clone(),
                message: "shortcut trigger cannot be empty".to_owned(),
            });
        }

        Ok(
            NewShortcut::new(self.action.portal_id(), self.action.description())
                .preferred_trigger(preferred_trigger),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed(HotkeyPress),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyPress {
    pub action: HotkeyAction,
    pub accelerator: String,
}

#[derive(Debug, Error)]
pub enum HotkeyWatcherError {
    #[error("failed to initialize global shortcuts portal: {0}")]
    Portal(String),

    #[error("invalid hotkey '{accelerator}': {message}")]
    InvalidAccelerator {
        accelerator: String,
        message: String,
    },

    #[error("hotkey watcher stopped before it finished starting")]
    StartupStopped,
}

pub struct HotkeyWatcherHandle {
    shutdown_tx: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl HotkeyWatcherHandle {
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        let _ = self.shutdown_tx.send(());

        if let Some(thread) = self.thread.take()
            && let Err(err) = thread.join()
        {
            log::warn!("hotkey watcher thread panicked: {err:?}");
        }
    }
}

impl Drop for HotkeyWatcherHandle {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

pub struct GlobalHotkeyWatcher;

impl GlobalHotkeyWatcher {
    pub fn spawn(
        bindings: Vec<HotkeyBinding>,
        event_tx: Sender<ServiceEvent>,
    ) -> Result<HotkeyWatcherHandle, HotkeyWatcherError> {
        log::debug!(
            "starting global shortcuts portal watcher with {} binding(s)",
            bindings.len()
        );

        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let (startup_tx, startup_rx) = mpsc::channel();

        let thread = thread::Builder::new()
            .name("wf-info-hotkey-watcher".to_owned())
            .spawn(move || {
                async_io::block_on(run_portal_hotkey_watcher(
                    bindings,
                    event_tx,
                    shutdown_rx,
                    startup_tx,
                ));
            })
            .map_err(|err| HotkeyWatcherError::Portal(err.to_string()))?;

        match startup_rx.recv() {
            Ok(Ok(())) => {
                log::debug!("global shortcuts portal watcher started");
                Ok(HotkeyWatcherHandle {
                    shutdown_tx,
                    thread: Some(thread),
                })
            }
            Ok(Err(err)) => {
                log::debug!("global shortcuts portal watcher failed during startup: {err}");
                let _ = thread.join();
                Err(err)
            }
            Err(_) => {
                log::debug!("global shortcuts portal watcher stopped before startup completed");
                let _ = thread.join();
                Err(HotkeyWatcherError::StartupStopped)
            }
        }
    }
}

pub fn phase_one_hotkey_bindings(settings: &HotkeyConfig) -> Vec<HotkeyBinding> {
    vec![
        HotkeyBinding::new(HotkeyAction::TriggerRewardScan, &settings.activation),
        HotkeyBinding::new(HotkeyAction::DismissOverlay, &settings.dismiss_overlay),
    ]
}

async fn run_portal_hotkey_watcher(
    bindings: Vec<HotkeyBinding>,
    event_tx: Sender<ServiceEvent>,
    shutdown_rx: mpsc::Receiver<()>,
    startup_tx: Sender<Result<(), HotkeyWatcherError>>,
) {
    log::debug!("binding global shortcuts through xdg-desktop-portal");

    let startup = bind_portal_shortcuts(&bindings).await;
    let Ok((portal, _session, actions_by_id)) = startup else {
        let _ = startup_tx.send(startup.map(|_| ()));
        return;
    };

    log::debug!(
        "bound {} global shortcut action(s); subscribing to activation signal",
        actions_by_id.len()
    );

    let activated_stream = portal.receive_activated().await.map_err(portal_error);
    let Ok(activated_stream) = activated_stream else {
        let _ = startup_tx.send(activated_stream.map(|_| ()));
        return;
    };

    let _ = startup_tx.send(Ok(()));
    pin_mut!(activated_stream);

    loop {
        if shutdown_rx.try_recv().is_ok() {
            log::debug!("global shortcuts portal watcher received shutdown signal");
            break;
        }

        let activation = activated_stream.next().fuse();
        let shutdown_tick = FutureExt::fuse(async_io::Timer::after(HOTKEY_SHUTDOWN_POLL_INTERVAL));
        pin_mut!(activation, shutdown_tick);

        select! {
            activation = activation => {
                let Some(activation) = activation else {
                    break;
                };

                if let Some(binding) = actions_by_id.get(activation.shortcut_id())
                {
                    log::debug!(
                        "global shortcut activated: id={}, action={:?}",
                        activation.shortcut_id(),
                        binding.action
                    );

                    if event_tx
                        .send(ServiceEvent::Hotkey(HotkeyEvent::Pressed(HotkeyPress {
                            action: binding.action,
                            accelerator: binding.accelerator.clone(),
                        })))
                        .is_err()
                    {
                        log::debug!("global shortcuts portal receiver dropped");
                        break;
                    }
                }
            }
            _ = shutdown_tick => {}
        }
    }

    log::debug!("global shortcuts portal watcher stopped");
}

async fn bind_portal_shortcuts(
    bindings: &[HotkeyBinding],
) -> Result<
    (
        GlobalShortcuts,
        ashpd::desktop::Session<GlobalShortcuts>,
        HashMap<String, HotkeyBinding>,
    ),
    HotkeyWatcherError,
> {
    log::debug!(
        "preparing {} global shortcut portal binding(s)",
        bindings.len()
    );

    let shortcuts = bindings
        .iter()
        .map(HotkeyBinding::portal_shortcut)
        .collect::<Result<Vec<_>, _>>()?;
    let actions_by_id = bindings
        .iter()
        .map(|binding| (binding.action.portal_id().to_owned(), binding.clone()))
        .collect();
    log::debug!("creating global shortcuts portal proxy");
    let portal = GlobalShortcuts::new().await.map_err(portal_error)?;
    log::debug!("creating global shortcuts portal session");
    let session = portal
        .create_session(CreateSessionOptions::default())
        .await
        .map_err(portal_error)?;
    log::debug!("requesting portal shortcut bindings");
    let request = portal
        .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
        .await
        .map_err(portal_error)?;

    request.response().map_err(portal_error)?;
    log::debug!("portal shortcut binding request completed");

    Ok((portal, session, actions_by_id))
}

fn portal_error(err: ashpd::Error) -> HotkeyWatcherError {
    HotkeyWatcherError::Portal(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{HotkeyAction, HotkeyBinding, HotkeyWatcherError, phase_one_hotkey_bindings};
    use crate::config::HotkeyConfig;

    #[test]
    fn phase_one_bindings_map_settings_to_actions() {
        let settings = HotkeyConfig {
            activation: "F12".to_owned(),
            dismiss_overlay: "control+F11".to_owned(),
        };

        let bindings = phase_one_hotkey_bindings(&settings);

        assert_eq!(
            bindings,
            vec![
                HotkeyBinding::new(HotkeyAction::TriggerRewardScan, "F12"),
                HotkeyBinding::new(HotkeyAction::DismissOverlay, "control+F11"),
            ]
        );
    }

    #[test]
    fn hotkey_bindings_build_portal_shortcuts() {
        let binding = HotkeyBinding::new(HotkeyAction::TriggerRewardScan, "F12");

        assert!(binding.portal_shortcut().is_ok());
    }

    #[test]
    fn empty_hotkey_bindings_report_the_configured_accelerator() {
        let binding = HotkeyBinding::new(HotkeyAction::TriggerRewardScan, " ");

        let err = binding
            .portal_shortcut()
            .expect_err("empty preferred trigger should fail");

        assert!(matches!(
            err,
            HotkeyWatcherError::InvalidAccelerator { accelerator, .. }
                if accelerator == " "
        ));
    }
}
