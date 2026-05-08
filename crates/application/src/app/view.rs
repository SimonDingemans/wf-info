use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, slider, text, text_input,
    tooltip,
};
use iced::{Element, Length};
use shared::config::Settings as AppSettings;
use shared::rewards::RewardOverlayEntry;

use super::message::Message;
use super::monitor::MonitorChoice;
use super::settings::{HotkeyCaptureTarget, Page, SettingsTab};
use super::state::Application;

const LOCALE_OPTIONS: &[&str] = &["en", "de", "es", "fr", "it", "nl", "pl", "pt", "ru", "tr"];
const CAPTURE_METHOD_OPTIONS: &[&str] = &["portal", "fixture"];
const DISPLAY_MODE_OPTIONS: &[&str] = &["borderless_fullscreen"];
const ASPECT_RATIO_OPTIONS: &[&str] = &["16:9"];
const OCR_LANGUAGE_OPTIONS: &[&str] = &["eng", "deu", "fra", "ita", "spa", "pol", "por", "rus"];
const WARFRAME_THEME_OPTIONS: &[&str] = &[
    "lotus",
    "vitruvian",
    "stalker",
    "baruuk",
    "corpus",
    "fortuna",
    "grineer",
    "nidus",
    "orokin",
    "tenno",
    "high_contrast",
    "legacy",
    "equinox",
    "dark_lotus",
    "zephyr",
];
const LOG_LEVEL_OPTIONS: &[&str] = &["error", "warn", "info", "debug", "trace"];

impl Application {
    pub(super) fn view(&self) -> Element<'_, Message> {
        match self.page {
            Page::Launcher => self.launcher_view(),
            Page::Settings => self.settings_view(),
        }
    }

    fn launcher_view(&self) -> Element<'_, Message> {
        let scan_button = button("Scan Now").padding([10, 14]).on_press_maybe(
            (self.settings.scanner.enabled && !self.reward_scan_in_progress)
                .then_some(Message::ScanNow),
        );
        let refresh_data_button = button("Refresh Data Cache")
            .padding([10, 14])
            .on_press_maybe(
                (!self.data_cache_refresh_in_progress).then_some(Message::RefreshDataCache),
            );
        let settings_button = button("Settings")
            .padding([10, 14])
            .on_press(Message::OpenSettings);

        let content = column![
            row![
                column![
                    text("wf-info").size(32),
                    text(format!("v{}", env!("CARGO_PKG_VERSION"))).size(16),
                ]
                .spacing(2),
                text(&self.status).size(16),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
            row![scan_button, refresh_data_button, settings_button]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            row![self.readiness_panel(), self.data_cache_panel()]
                .spacing(16)
                .width(Length::Fill),
            self.last_scan_panel(),
            self.diagnostics_panel(),
        ]
        .spacing(16)
        .padding(24)
        .width(Length::Fill);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn readiness_panel(&self) -> Element<'_, Message> {
        let scanner_state = if self.reward_scan_in_progress {
            "Scanning"
        } else if self.settings.scanner.enabled {
            "Ready"
        } else {
            "Disabled"
        };
        let log_state = if !self.settings.scanner.enabled {
            "Disabled"
        } else if self.settings.warframe.log_path.trim().is_empty() {
            "Discovering EE.log"
        } else {
            "Watching EE.log"
        };
        let selected_monitor = self
            .selected_monitor
            .as_ref()
            .map(|monitor| monitor.label.clone())
            .unwrap_or_else(|| "No detected monitor".to_owned());

        dashboard_section(
            "Scanner Readiness",
            column![
                detail_row("Scanner", scanner_state),
                detail_row("Automatic detection", log_state),
                detail_row("Activation hotkey", &self.settings.hotkeys.activation),
                detail_row("Overlay", enabled_label(self.settings.overlay.enabled)),
                detail_row("Clipboard", enabled_label(self.settings.clipboard.enabled)),
                detail_row("Capture method", &self.settings.capture.capture_method),
                detail_row("Configured monitor", &self.settings.capture.monitor),
                detail_row("Selected monitor", selected_monitor),
            ],
        )
    }

    fn data_cache_panel(&self) -> Element<'_, Message> {
        let mut rows = column![].spacing(8);

        rows = rows.push(detail_row(
            "Refresh state",
            if self.data_cache_refresh_in_progress {
                "Refreshing"
            } else {
                "Idle"
            },
        ));

        rows = match &self.last_data_cache_refresh {
            Some(Ok(refresh)) => refresh.payloads().into_iter().fold(
                rows.push(detail_row("Last result", "Refreshed"))
                    .push(detail_row(
                        "Remote payloads",
                        refresh.remote_count().to_string(),
                    ))
                    .push(detail_row(
                        "Local fallbacks",
                        refresh.local_fallback_count().to_string(),
                    )),
                |rows, (label, payload)| {
                    rows.push(detail_row(
                        format!("{label} ({})", payload.source.label()),
                        payload.path.display().to_string(),
                    ))
                },
            ),
            Some(Err(err)) => rows
                .push(detail_row("Last result", "Failed"))
                .push(text(err).size(14)),
            None => rows.push(detail_row(
                "Last result",
                "No refresh completed this session",
            )),
        };

        dashboard_section("Data Cache", rows)
    }

    fn last_scan_panel(&self) -> Element<'_, Message> {
        let rows = if self.reward_scan_in_progress {
            column![text("Reward scan is running.").size(14)].spacing(8)
        } else {
            match &self.last_reward_scan {
                Some(Ok(rewards)) if rewards.is_empty() => {
                    column![text("Last scan completed, but no rewards were found.").size(14)]
                        .spacing(8)
                }
                Some(Ok(rewards)) => rewards.iter().fold(
                    column![text(format!("Last scan found {} reward(s).", rewards.len())).size(14)]
                        .spacing(8),
                    |column, reward| column.push(reward_row(reward)),
                ),
                Some(Err(err)) => {
                    column![detail_row("Last result", "Failed"), text(err).size(14),].spacing(8)
                }
                None => column![text("No reward scan completed this session.").size(14)].spacing(8),
            }
        };

        dashboard_section("Last Reward Scan", rows)
    }

    fn diagnostics_panel(&self) -> Element<'_, Message> {
        let toggle_label = if self.diagnostics_expanded {
            "Hide Diagnostics"
        } else {
            "Show Diagnostics"
        };
        let mut body = column![
            row![
                text("Diagnostics").size(20),
                button(toggle_label)
                    .padding([8, 12])
                    .on_press(Message::ToggleDiagnostics),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center)
        ]
        .spacing(12);

        if self.diagnostics_expanded {
            let detect_button = button("Show Monitor Info Overlays")
                .padding([10, 14])
                .on_press_maybe((!self.busy).then_some(Message::DetectMonitorInfo));
            let test_overlay_button = button("Draw Test Overlay")
                .padding([10, 14])
                .on_press(Message::DrawTestOverlay);
            let quit_overlays_button = button("Quit Debug Overlays")
                .padding([10, 14])
                .on_press(Message::QuitDebugOverlays);
            let selected = self.selected_monitor.clone();
            let monitor_picker = pick_list(
                self.monitors.as_slice(),
                selected,
                Message::SelectedMonitorChanged,
            )
            .placeholder("Active screen");
            let clipboard_toggle = checkbox(
                "Copy reward summaries after scans",
                self.settings.clipboard.enabled,
            )
            .on_toggle(Message::ClipboardOutputChanged);

            body = body
                .push(
                    row![detect_button, test_overlay_button, quit_overlays_button]
                        .spacing(12)
                        .align_y(iced::Alignment::Center),
                )
                .push(
                    row![text("Selected capture/overlay target:"), monitor_picker]
                        .spacing(12)
                        .align_y(iced::Alignment::Center),
                )
                .push(clipboard_toggle)
                .push(monitor_details(&self.monitors));
        }

        container(body).padding(14).width(Length::Fill).into()
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let tabs = SettingsTab::ALL.iter().fold(row![].spacing(8), |row, tab| {
            row.push(
                button(tab.label())
                    .padding([8, 12])
                    .on_press(Message::SettingsTabSelected(*tab)),
            )
        });
        let actions = row![
            button("Save")
                .padding([10, 14])
                .on_press(Message::SaveSettings),
            button("Cancel")
                .padding([10, 14])
                .on_press(Message::CancelSettings),
        ]
        .spacing(12);
        let settings_form = settings_tab_view(
            self.settings_tab,
            &self.settings_draft,
            &self.monitors,
            self.capturing_hotkey,
        );

        let content = column![
            row![text("Settings").size(32), actions]
                .spacing(16)
                .align_y(iced::Alignment::Center),
            tabs,
            scrollable(settings_form).height(Length::Fill),
            text(&self.status),
        ]
        .spacing(16)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn dashboard_section<'a>(
    title: &'static str,
    fields: iced::widget::Column<'a, Message>,
) -> Element<'a, Message> {
    container(column![text(title).size(20), fields.spacing(8)].spacing(10))
        .padding(14)
        .width(Length::Fill)
        .into()
}

fn detail_row<'a>(label: impl Into<String>, value: impl Into<String>) -> Element<'a, Message> {
    row![
        text(label.into()).size(14).width(Length::Fixed(160.0)),
        text(value.into()).size(14),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

fn enabled_label(enabled: bool) -> &'static str {
    if enabled { "Enabled" } else { "Disabled" }
}

fn reward_row<'a>(reward: &RewardOverlayEntry) -> Element<'a, Message> {
    let mut metrics = Vec::new();

    if let Some(platinum) = reward.platinum {
        metrics.push(format!("{platinum}p"));
    }
    if let Some(ducats) = reward.ducats {
        metrics.push(format!("{ducats} ducats"));
    }
    if let Some(volume) = reward.volume {
        metrics.push(format!("{volume} volume"));
    }
    if reward.vaulted {
        metrics.push("vaulted".to_owned());
    }
    if reward.mastered {
        metrics.push("mastered".to_owned());
    }
    if let (Some(owned), Some(required)) = (reward.owned_count, reward.required_count) {
        metrics.push(format!("{owned}/{required} owned"));
    }

    let summary = if metrics.is_empty() {
        "No market data".to_owned()
    } else {
        metrics.join(" / ")
    };

    row![
        text(reward.name.clone())
            .size(14)
            .width(Length::FillPortion(2)),
        text(summary).size(14).width(Length::FillPortion(3)),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center)
    .into()
}

fn monitor_details<'a>(monitors: &'a [MonitorChoice]) -> Element<'a, Message> {
    if monitors.is_empty() {
        return text("No monitor information detected.").size(14).into();
    }

    monitors
        .iter()
        .fold(column![].spacing(10), |column, monitor| {
            let details = monitor.info.summary_lines().into_iter().fold(
                column![text(monitor.label.clone()).size(16)].spacing(2),
                |column, line| column.push(text(line).size(14)),
            );

            column.push(details)
        })
        .into()
}

fn settings_tab_view<'a>(
    tab: SettingsTab,
    settings: &'a AppSettings,
    monitors: &'a [MonitorChoice],
    capturing_hotkey: Option<HotkeyCaptureTarget>,
) -> Element<'a, Message> {
    match tab {
        SettingsTab::App => section(
            "App",
            column![
                pick_field(
                    "Locale",
                    "Language used for app text and item-name matching.",
                    &settings.app.locale,
                    LOCALE_OPTIONS,
                    Message::SettingsAppLocaleChanged
                ),
                checkbox_field(
                    "Start minimized",
                    "Start the launcher without foregrounding the main window.",
                    settings.app.start_minimized,
                    Message::SettingsAppStartMinimizedChanged
                ),
            ],
        ),
        SettingsTab::Capture => section(
            "Capture",
            column![
                monitor_pick_field(
                    "Monitor/output",
                    "Configured output used for capture and overlay placement.",
                    settings,
                    monitors
                ),
                pick_field(
                    "Capture method",
                    "Wayland portal is the normal capture path; fixture uses the bundled test image.",
                    &settings.capture.capture_method,
                    CAPTURE_METHOD_OPTIONS,
                    Message::SettingsCaptureMethodChanged
                ),
                pick_field(
                    "Display mode",
                    "Only borderless fullscreen Warframe is supported in Phase 1.",
                    &settings.capture.display_mode,
                    DISPLAY_MODE_OPTIONS,
                    Message::SettingsDisplayModeChanged
                ),
                pick_field(
                    "Aspect ratio",
                    "Reward-screen detection currently assumes a 16:9 game frame.",
                    &settings.capture.aspect_ratio,
                    ASPECT_RATIO_OPTIONS,
                    Message::SettingsAspectRatioChanged
                ),
            ],
        ),
        SettingsTab::Scanner => section(
            "Scanner",
            column![
                checkbox_field(
                    "Enable automatic reward scanner",
                    "Listen for Warframe reward-screen log markers and scan automatically.",
                    settings.scanner.enabled,
                    Message::SettingsScannerEnabledChanged
                ),
                u32_slider_field(
                    "Automatic OCR delay (ms)",
                    "Wait this long after the log marker before capturing the reward screen.",
                    settings.scanner.auto_delay_ms as u32,
                    0..=5_000,
                    50,
                    Message::SettingsScannerAutoDelayChanged
                ),
                checkbox_field(
                    "Save debug images",
                    "Write full captures and OCR crops for reward-scan debugging.",
                    settings.scanner.debug_images,
                    Message::SettingsScannerDebugImagesChanged
                ),
                u32_slider_field(
                    "Debug image retention (hours)",
                    "Delete old reward debug captures after this many hours.",
                    settings.scanner.debug_image_retention_hours as u32,
                    0..=168,
                    1,
                    Message::SettingsScannerRetentionChanged
                ),
            ],
        ),
        SettingsTab::Hotkeys => section(
            "Hotkeys",
            column![
                hotkey_field(
                    "Activation hotkey",
                    "Click the button, then press the key or key combination that should trigger reward scanning.",
                    &settings.hotkeys.activation,
                    HotkeyCaptureTarget::Activation,
                    capturing_hotkey
                ),
                hotkey_field(
                    "Dismiss overlay hotkey",
                    "Click the button, then press the key or key combination that should dismiss reward overlays.",
                    &settings.hotkeys.dismiss_overlay,
                    HotkeyCaptureTarget::DismissOverlay,
                    capturing_hotkey
                ),
            ],
        ),
        SettingsTab::Overlay => section(
            "Overlay",
            column![
                checkbox_field(
                    "Show reward overlay",
                    "Display reward results in the layer-shell overlay after scans.",
                    settings.overlay.enabled,
                    Message::SettingsOverlayEnabledChanged
                ),
                i32_slider_field(
                    "X offset",
                    "Move the reward overlay horizontally from its detected position.",
                    settings.overlay.x_offset,
                    -500..=500,
                    1,
                    Message::SettingsOverlayXOffsetChanged
                ),
                i32_slider_field(
                    "Y offset",
                    "Move the reward overlay vertically from its detected position.",
                    settings.overlay.y_offset,
                    -500..=500,
                    1,
                    Message::SettingsOverlayYOffsetChanged
                ),
                u32_slider_field(
                    "Auto-hide delay (ms)",
                    "Keep the reward overlay visible for this duration.",
                    settings.overlay.duration_ms as u32,
                    1_000..=60_000,
                    500,
                    Message::SettingsOverlayDurationChanged
                ),
                checkbox_field(
                    "High contrast overlay",
                    "Use a stronger overlay background for readability.",
                    settings.overlay.high_contrast,
                    Message::SettingsOverlayHighContrastChanged
                ),
            ],
        ),
        SettingsTab::Clipboard => section(
            "Clipboard",
            column![
                checkbox_field(
                    "Copy reward summaries after scans",
                    "Copy a plain-text reward summary when OCR succeeds.",
                    settings.clipboard.enabled,
                    Message::SettingsClipboardEnabledChanged
                ),
                checkbox_field(
                    "Include vaulted marker",
                    "Append a vaulted note for vaulted reward parts in clipboard output.",
                    settings.clipboard.include_vaulted_marker,
                    Message::SettingsClipboardVaultedMarkerChanged
                ),
                text_field(
                    "Footer",
                    "Optional text appended to copied reward summaries.",
                    &settings.clipboard.footer,
                    Message::SettingsClipboardFooterChanged
                ),
            ],
        ),
        SettingsTab::Ocr => section(
            "OCR",
            column![
                pick_field(
                    "Language",
                    "Tesseract language code used for reward-name OCR.",
                    &settings.ocr.language,
                    OCR_LANGUAGE_OPTIONS,
                    Message::SettingsOcrLanguageChanged
                ),
                text_field(
                    "Tesseract data path",
                    "Optional custom tessdata path; leave empty to use the system default.",
                    &settings.ocr.tesseract_data_path,
                    Message::SettingsTesseractDataPathChanged
                ),
                f32_slider_field(
                    "Confidence threshold",
                    "Discard OCR candidates below this confidence score.",
                    settings.ocr.confidence_threshold,
                    0.0..=100.0,
                    1.0,
                    Message::SettingsOcrConfidenceChanged
                ),
            ],
        ),
        SettingsTab::Warframe => section(
            "Warframe",
            column![
                text_field(
                    "EE.log path",
                    "Optional explicit Warframe EE.log path; leave empty to auto-discover Steam/Proton locations.",
                    &settings.warframe.log_path,
                    Message::SettingsWarframeLogPathChanged
                ),
                pick_field(
                    "UI theme",
                    "Warframe UI color theme used by reward-box extraction.",
                    &settings.warframe.ui_theme,
                    WARFRAME_THEME_OPTIONS,
                    Message::SettingsWarframeUiThemeChanged
                ),
            ],
        ),
        SettingsTab::Logging => section(
            "Logging",
            column![
                pick_field(
                    "Log level",
                    "Minimum application log level for terminal and file logging.",
                    &settings.logging.level,
                    LOG_LEVEL_OPTIONS,
                    Message::SettingsLoggingLevelChanged
                ),
                text_field(
                    "Log file",
                    "Optional explicit log file path; leave empty for the default app log path.",
                    &settings.logging.file,
                    Message::SettingsLoggingFileChanged
                ),
            ],
        ),
    }
}

fn section<'a>(
    title: &'static str,
    fields: iced::widget::Column<'a, Message>,
) -> Element<'a, Message> {
    container(column![text(title).size(24), fields.spacing(12)].spacing(16))
        .padding(4)
        .width(Length::Fill)
        .into()
}

fn text_field<'a>(
    label: &'static str,
    help: &'static str,
    value: &str,
    on_input: fn(String) -> Message,
) -> Element<'a, Message> {
    column![
        label_with_info(label, help),
        text_input(label, value)
            .on_input(on_input)
            .padding([8, 10])
            .width(Length::Fill),
    ]
    .spacing(4)
    .into()
}

fn pick_field<'a>(
    label: &'static str,
    help: &'static str,
    value: &str,
    options: &'static [&'static str],
    on_selected: fn(String) -> Message,
) -> Element<'a, Message> {
    column![
        label_with_info(label, help),
        pick_list(options, selected_option(value, options), move |value| {
            on_selected(value.to_owned())
        })
        .placeholder(label)
        .width(Length::Fill),
    ]
    .spacing(4)
    .into()
}

fn monitor_pick_field<'a>(
    label: &'static str,
    help: &'static str,
    settings: &AppSettings,
    monitors: &[MonitorChoice],
) -> Element<'a, Message> {
    let options = monitor_target_options(settings, monitors);
    let selected =
        (!settings.capture.monitor.trim().is_empty()).then(|| settings.capture.monitor.clone());

    column![
        label_with_info(label, help),
        pick_list(options, selected, Message::SettingsCaptureMonitorChanged)
            .placeholder("No detected monitor")
            .width(Length::Fill),
    ]
    .spacing(4)
    .into()
}

fn monitor_target_options(settings: &AppSettings, monitors: &[MonitorChoice]) -> Vec<String> {
    let mut options = vec!["primary".to_owned()];

    for output_name in monitors
        .iter()
        .filter_map(|monitor| monitor.output_name.as_ref())
    {
        if !options.iter().any(|option| option == output_name) {
            options.push(output_name.clone());
        }
    }

    let configured = settings.capture.monitor.trim();
    if !configured.is_empty() && !options.iter().any(|option| option == configured) {
        options.push(configured.to_owned());
    }

    options
}

fn selected_option(value: &str, options: &'static [&'static str]) -> Option<&'static str> {
    options
        .iter()
        .copied()
        .find(|option| option.eq_ignore_ascii_case(value))
}

fn u32_slider_field<'a>(
    label: &'static str,
    help: &'static str,
    value: u32,
    range: std::ops::RangeInclusive<u32>,
    step: u32,
    on_change: fn(u32) -> Message,
) -> Element<'a, Message> {
    column![
        row![
            label_with_info(label, help),
            text(value.to_string()).size(14)
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
        slider(range, value, on_change).step(step),
    ]
    .spacing(4)
    .into()
}

fn i32_slider_field<'a>(
    label: &'static str,
    help: &'static str,
    value: i32,
    range: std::ops::RangeInclusive<i32>,
    step: i32,
    on_change: fn(i32) -> Message,
) -> Element<'a, Message> {
    column![
        row![
            label_with_info(label, help),
            text(value.to_string()).size(14)
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
        slider(range, value, on_change).step(step),
    ]
    .spacing(4)
    .into()
}

fn f32_slider_field<'a>(
    label: &'static str,
    help: &'static str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    on_change: fn(f32) -> Message,
) -> Element<'a, Message> {
    column![
        row![
            label_with_info(label, help),
            text(format!("{value:.1}")).size(14)
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
        slider(range, value, on_change).step(step),
    ]
    .spacing(4)
    .into()
}

fn checkbox_field<'a>(
    label: &'static str,
    help: &'static str,
    checked: bool,
    on_toggle: fn(bool) -> Message,
) -> Element<'a, Message> {
    row![
        checkbox(label, checked).on_toggle(on_toggle),
        info_icon(help),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

fn hotkey_field<'a>(
    label: &'static str,
    help: &'static str,
    value: &'a str,
    target: HotkeyCaptureTarget,
    capturing_hotkey: Option<HotkeyCaptureTarget>,
) -> Element<'a, Message> {
    let button_label = if capturing_hotkey == Some(target) {
        "Press key combination..."
    } else {
        value
    };

    column![
        label_with_info(label, help),
        button(button_label)
            .padding([8, 10])
            .width(Length::Fill)
            .on_press(Message::StartHotkeyCapture(target)),
    ]
    .spacing(4)
    .into()
}

fn label_with_info<'a>(label: &'static str, help: &'static str) -> Element<'a, Message> {
    row![text(label).size(14), info_icon(help)]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .into()
}

fn info_icon<'a>(help: &'static str) -> Element<'a, Message> {
    tooltip(
        text("[?]").size(14),
        container(text(help).size(14)).padding(10),
        tooltip::Position::Right,
    )
    .into()
}
