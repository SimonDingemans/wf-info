use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Length};
use shared::config::Settings as AppSettings;

use super::message::Message;
use super::settings::{Page, SettingsTab};
use super::state::Application;

impl Application {
    pub(super) fn view(&self) -> Element<'_, Message> {
        match self.page {
            Page::Launcher => self.launcher_view(),
            Page::Settings => self.settings_view(),
        }
    }

    fn launcher_view(&self) -> Element<'_, Message> {
        let detect_button = button("Show Monitor Info Overlays")
            .padding([10, 14])
            .on_press_maybe((!self.busy).then_some(Message::DetectMonitorInfo));

        let test_overlay_button = button("Draw Test Overlay")
            .padding([10, 14])
            .on_press(Message::DrawTestOverlay);
        let quit_overlays_button = button("Quit Debug Overlays")
            .padding([10, 14])
            .on_press(Message::QuitDebugOverlays);
        let refresh_data_button = button("Refresh Data Cache")
            .padding([10, 14])
            .on_press_maybe(
                (!self.data_cache_refresh_in_progress).then_some(Message::RefreshDataCache),
            );
        let settings_button = button("Settings")
            .padding([10, 14])
            .on_press(Message::OpenSettings);

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

        let monitor_rows = self
            .monitors
            .iter()
            .fold(column![].spacing(8), |column, monitor| {
                let details = monitor
                    .info
                    .summary_lines()
                    .into_iter()
                    .fold(column![].spacing(2), |column, line| column.push(text(line)));

                column.push(container(details).padding(12).width(Length::Fill))
            });

        let content = column![
            text("wf-info").size(32),
            text("Basic application shell").size(18),
            row![
                detect_button,
                test_overlay_button,
                quit_overlays_button,
                refresh_data_button,
                settings_button
            ]
            .spacing(12),
            row![text("Selected capture/overlay target:"), monitor_picker]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            clipboard_toggle,
            text(&self.status),
            scrollable(monitor_rows).height(Length::Fill),
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
        let settings_form = settings_tab_view(self.settings_tab, &self.settings_draft);

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

fn settings_tab_view(tab: SettingsTab, settings: &AppSettings) -> Element<'_, Message> {
    match tab {
        SettingsTab::App => section(
            "App",
            column![
                text_field(
                    "Locale",
                    &settings.app.locale,
                    Message::SettingsAppLocaleChanged
                ),
                checkbox("Start minimized", settings.app.start_minimized)
                    .on_toggle(Message::SettingsAppStartMinimizedChanged),
            ],
        ),
        SettingsTab::Capture => section(
            "Capture",
            column![
                text_field(
                    "Monitor/output",
                    &settings.capture.monitor,
                    Message::SettingsCaptureMonitorChanged
                ),
                text_field(
                    "Capture method",
                    &settings.capture.capture_method,
                    Message::SettingsCaptureMethodChanged
                ),
                text_field(
                    "Display mode",
                    &settings.capture.display_mode,
                    Message::SettingsDisplayModeChanged
                ),
                text_field(
                    "Aspect ratio",
                    &settings.capture.aspect_ratio,
                    Message::SettingsAspectRatioChanged
                ),
            ],
        ),
        SettingsTab::Scanner => section(
            "Scanner",
            column![
                checkbox("Enable automatic reward scanner", settings.scanner.enabled)
                    .on_toggle(Message::SettingsScannerEnabledChanged),
                text_field(
                    "Automatic OCR delay (ms)",
                    &settings.scanner.auto_delay_ms.to_string(),
                    Message::SettingsScannerAutoDelayChanged
                ),
                checkbox("Save debug images", settings.scanner.debug_images)
                    .on_toggle(Message::SettingsScannerDebugImagesChanged),
                text_field(
                    "Debug image retention (hours)",
                    &settings.scanner.debug_image_retention_hours.to_string(),
                    Message::SettingsScannerRetentionChanged
                ),
            ],
        ),
        SettingsTab::Hotkeys => section(
            "Hotkeys",
            column![
                text_field(
                    "Activation hotkey",
                    &settings.hotkeys.activation,
                    Message::SettingsActivationHotkeyChanged
                ),
                text_field(
                    "Dismiss overlay hotkey",
                    &settings.hotkeys.dismiss_overlay,
                    Message::SettingsDismissOverlayHotkeyChanged
                ),
            ],
        ),
        SettingsTab::Overlay => section(
            "Overlay",
            column![
                checkbox("Show reward overlay", settings.overlay.enabled)
                    .on_toggle(Message::SettingsOverlayEnabledChanged),
                text_field(
                    "X offset",
                    &settings.overlay.x_offset.to_string(),
                    Message::SettingsOverlayXOffsetChanged
                ),
                text_field(
                    "Y offset",
                    &settings.overlay.y_offset.to_string(),
                    Message::SettingsOverlayYOffsetChanged
                ),
                text_field(
                    "Auto-hide delay (ms)",
                    &settings.overlay.duration_ms.to_string(),
                    Message::SettingsOverlayDurationChanged
                ),
                checkbox("High contrast overlay", settings.overlay.high_contrast)
                    .on_toggle(Message::SettingsOverlayHighContrastChanged),
            ],
        ),
        SettingsTab::Clipboard => section(
            "Clipboard",
            column![
                checkbox(
                    "Copy reward summaries after scans",
                    settings.clipboard.enabled
                )
                .on_toggle(Message::SettingsClipboardEnabledChanged),
                checkbox(
                    "Include vaulted marker",
                    settings.clipboard.include_vaulted_marker
                )
                .on_toggle(Message::SettingsClipboardVaultedMarkerChanged),
                text_field(
                    "Footer",
                    &settings.clipboard.footer,
                    Message::SettingsClipboardFooterChanged
                ),
            ],
        ),
        SettingsTab::Ocr => section(
            "OCR",
            column![
                text_field(
                    "Language",
                    &settings.ocr.language,
                    Message::SettingsOcrLanguageChanged
                ),
                text_field(
                    "Tesseract data path",
                    &settings.ocr.tesseract_data_path,
                    Message::SettingsTesseractDataPathChanged
                ),
                text_field(
                    "Confidence threshold",
                    &settings.ocr.confidence_threshold.to_string(),
                    Message::SettingsOcrConfidenceChanged
                ),
            ],
        ),
        SettingsTab::Warframe => section(
            "Warframe",
            column![
                text_field(
                    "EE.log path",
                    &settings.warframe.log_path,
                    Message::SettingsWarframeLogPathChanged
                ),
                text_field(
                    "UI theme",
                    &settings.warframe.ui_theme,
                    Message::SettingsWarframeUiThemeChanged
                ),
            ],
        ),
        SettingsTab::Logging => section(
            "Logging",
            column![
                text_field(
                    "Log level",
                    &settings.logging.level,
                    Message::SettingsLoggingLevelChanged
                ),
                text_field(
                    "Log file",
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
    value: &str,
    on_input: fn(String) -> Message,
) -> Element<'a, Message> {
    column![
        text(label).size(14),
        text_input(label, value)
            .on_input(on_input)
            .padding([8, 10])
            .width(Length::Fill),
    ]
    .spacing(4)
    .into()
}
