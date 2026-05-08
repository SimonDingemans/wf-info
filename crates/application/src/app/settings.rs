use super::state::Application;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Page {
    Launcher,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SettingsTab {
    App,
    Capture,
    Scanner,
    Hotkeys,
    Overlay,
    Clipboard,
    Ocr,
    Warframe,
    Logging,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HotkeyCaptureTarget {
    Activation,
    DismissOverlay,
}

impl SettingsTab {
    pub(super) const ALL: [Self; 9] = [
        Self::App,
        Self::Capture,
        Self::Scanner,
        Self::Hotkeys,
        Self::Overlay,
        Self::Clipboard,
        Self::Ocr,
        Self::Warframe,
        Self::Logging,
    ];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::App => "App",
            Self::Capture => "Capture",
            Self::Scanner => "Scanner",
            Self::Hotkeys => "Hotkeys",
            Self::Overlay => "Overlay",
            Self::Clipboard => "Clipboard",
            Self::Ocr => "OCR",
            Self::Warframe => "Warframe",
            Self::Logging => "Logging",
        }
    }
}

impl Application {
    pub(super) fn set_clipboard_output_enabled(&mut self, enabled: bool) {
        self.settings.clipboard.enabled = enabled;
        self.settings_draft.clipboard.enabled = enabled;

        match self.context.save_settings(&self.settings) {
            Ok(()) if enabled => {
                self.status = "Clipboard summaries enabled.".to_owned();
            }
            Ok(()) => {
                self.status = "Clipboard summaries disabled.".to_owned();
            }
            Err(err) if enabled => {
                self.status = format!(
                    "Clipboard summaries enabled for this session, but could not save: {err}"
                );
            }
            Err(err) => {
                self.status = format!(
                    "Clipboard summaries disabled for this session, but could not save: {err}"
                );
            }
        }
    }

    pub(super) fn open_settings_page(&mut self) {
        self.settings_draft = self.settings.clone();
        self.page = Page::Settings;
        self.status = "Editing settings.".to_owned();
    }

    pub(super) fn cancel_settings_page(&mut self) {
        self.settings_draft = self.settings.clone();
        self.page = Page::Launcher;
        self.status = "Settings changes cancelled.".to_owned();
    }

    pub(super) fn select_settings_tab(&mut self, tab: SettingsTab) {
        self.settings_tab = tab;
    }

    pub(super) fn save_settings_page(&mut self) {
        if let Err(err) = self.settings_draft.capture.validate_supported() {
            self.status = format!("Settings not saved: {err}");
            self.settings_tab = SettingsTab::Capture;
            return;
        }

        match self.context.save_settings(&self.settings_draft) {
            Ok(()) => {
                self.settings = self.settings_draft.clone();
                self.page = Page::Launcher;
                self.status = "Settings saved.".to_owned();
            }
            Err(err) => {
                self.status = format!("Settings could not be saved: {err}");
            }
        }
    }

    pub(super) fn set_settings_app_locale(&mut self, value: String) {
        self.settings_draft.app.locale = value;
    }

    pub(super) fn set_settings_app_start_minimized(&mut self, value: bool) {
        self.settings_draft.app.start_minimized = value;
    }

    pub(super) fn set_settings_capture_monitor(&mut self, value: String) {
        self.settings_draft.capture.monitor = value;
    }

    pub(super) fn set_settings_capture_method(&mut self, value: String) {
        self.settings_draft.capture.capture_method = value;
    }

    pub(super) fn set_settings_display_mode(&mut self, value: String) {
        self.settings_draft.capture.display_mode = value;
    }

    pub(super) fn set_settings_aspect_ratio(&mut self, value: String) {
        self.settings_draft.capture.aspect_ratio = value;
    }

    pub(super) fn set_settings_scanner_enabled(&mut self, value: bool) {
        self.settings_draft.scanner.enabled = value;
    }

    pub(super) fn set_settings_scanner_auto_delay(&mut self, value: u32) {
        self.settings_draft.scanner.auto_delay_ms = u64::from(value);
    }

    pub(super) fn set_settings_scanner_debug_images(&mut self, value: bool) {
        self.settings_draft.scanner.debug_images = value;
    }

    pub(super) fn set_settings_scanner_retention(&mut self, value: u32) {
        self.settings_draft.scanner.debug_image_retention_hours = u64::from(value);
    }

    pub(super) fn begin_hotkey_capture(&mut self, target: HotkeyCaptureTarget) {
        self.capturing_hotkey = Some(target);
        self.status = match target {
            HotkeyCaptureTarget::Activation => {
                "Press the activation hotkey combination.".to_owned()
            }
            HotkeyCaptureTarget::DismissOverlay => {
                "Press the overlay dismissal hotkey combination.".to_owned()
            }
        };
    }

    pub(super) fn finish_hotkey_capture(&mut self, accelerator: String) {
        let Some(target) = self.capturing_hotkey.take() else {
            return;
        };

        match target {
            HotkeyCaptureTarget::Activation => {
                self.settings_draft.hotkeys.activation = accelerator;
            }
            HotkeyCaptureTarget::DismissOverlay => {
                self.settings_draft.hotkeys.dismiss_overlay = accelerator;
            }
        }

        self.status = "Hotkey captured. Save settings to apply it.".to_owned();
    }

    pub(super) fn cancel_hotkey_capture(&mut self) {
        if self.capturing_hotkey.take().is_some() {
            self.status = "Hotkey capture cancelled.".to_owned();
        }
    }

    pub(super) fn set_settings_overlay_enabled(&mut self, value: bool) {
        self.settings_draft.overlay.enabled = value;
    }

    pub(super) fn set_settings_overlay_x_offset(&mut self, value: i32) {
        self.settings_draft.overlay.x_offset = value;
    }

    pub(super) fn set_settings_overlay_y_offset(&mut self, value: i32) {
        self.settings_draft.overlay.y_offset = value;
    }

    pub(super) fn set_settings_overlay_duration(&mut self, value: u32) {
        self.settings_draft.overlay.duration_ms = u64::from(value);
    }

    pub(super) fn set_settings_overlay_high_contrast(&mut self, value: bool) {
        self.settings_draft.overlay.high_contrast = value;
    }

    pub(super) fn set_settings_clipboard_enabled(&mut self, value: bool) {
        self.settings_draft.clipboard.enabled = value;
    }

    pub(super) fn set_settings_clipboard_vaulted_marker(&mut self, value: bool) {
        self.settings_draft.clipboard.include_vaulted_marker = value;
    }

    pub(super) fn set_settings_clipboard_footer(&mut self, value: String) {
        self.settings_draft.clipboard.footer = value;
    }

    pub(super) fn set_settings_ocr_language(&mut self, value: String) {
        self.settings_draft.ocr.language = value;
    }

    pub(super) fn set_settings_tesseract_data_path(&mut self, value: String) {
        self.settings_draft.ocr.tesseract_data_path = value;
    }

    pub(super) fn set_settings_ocr_confidence(&mut self, value: f32) {
        self.settings_draft.ocr.confidence_threshold = value;
    }

    pub(super) fn set_settings_warframe_log_path(&mut self, value: String) {
        self.settings_draft.warframe.log_path = value;
    }

    pub(super) fn set_settings_warframe_ui_theme(&mut self, value: String) {
        self.settings_draft.warframe.ui_theme = value;
    }

    pub(super) fn set_settings_logging_level(&mut self, value: String) {
        self.settings_draft.logging.level = value;
    }

    pub(super) fn set_settings_logging_file(&mut self, value: String) {
        self.settings_draft.logging.file = value;
    }
}
