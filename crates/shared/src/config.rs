use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Config {
    #[serde(default)]
    pub app: AppConfig,
    #[serde(default)]
    pub capture: CaptureConfig,
    #[serde(default)]
    pub scanner: ScannerConfig,
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub overlay: OverlayConfig,
    #[serde(default)]
    pub clipboard: ClipboardConfig,
    #[serde(default)]
    pub ocr: OcrConfig,
    #[serde(default)]
    pub warframe: WarframeConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Config {
    pub fn with_cli_overrides(mut self, overrides: &CliConfigOverrides) -> Self {
        if let Some(level) = overrides.logging_level() {
            self.logging.level = level.to_owned();
        }
        self
    }

    pub fn path_for_app(app_name: &str) -> PathBuf {
        config_path(app_name)
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        match fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents).map_err(|err| err.to_string()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn read_or_create(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        if path.exists() {
            return Self::read(path);
        }

        let config = Self::default();
        config.write(path)?;
        Ok(config)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }

        let contents = toml::to_string_pretty(self).map_err(|err| err.to_string())?;
        let temporary_path = path.with_extension("toml.tmp");
        fs::write(&temporary_path, contents).map_err(|err| err.to_string())?;
        fs::rename(&temporary_path, path).map_err(|err| err.to_string())
    }

    pub fn capture_portal_restore_token(&self) -> Option<&str> {
        self.capture.portal_restore_token.as_deref()
    }

    pub fn set_capture_monitor(&mut self, monitor: impl Into<String>) {
        self.capture.monitor = monitor.into();
    }

    pub fn set_capture_portal_restore_token(&mut self, restore_token: impl Into<String>) {
        self.capture.portal_restore_token = Some(restore_token.into());
    }
}

pub type Settings = Config;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliConfigOverrides {
    logging_level: Option<String>,
}

impl CliConfigOverrides {
    pub fn set_logging_level(&mut self, level: impl Into<String>) {
        self.logging_level = Some(level.into());
    }

    pub fn logging_level(&self) -> Option<&str> {
        self.logging_level.as_deref()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct AppConfig {
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default)]
    pub start_minimized: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            locale: default_locale(),
            start_minimized: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CaptureConfig {
    #[serde(default = "default_monitor")]
    pub monitor: String,
    #[serde(default = "default_capture_method")]
    pub capture_method: String,
    #[serde(default = "default_display_mode")]
    pub display_mode: String,
    #[serde(default = "default_aspect_ratio")]
    pub aspect_ratio: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portal_restore_token: Option<String>,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            monitor: default_monitor(),
            capture_method: default_capture_method(),
            display_mode: default_display_mode(),
            aspect_ratio: default_aspect_ratio(),
            portal_restore_token: None,
        }
    }
}

impl CaptureConfig {
    pub fn validate_supported(&self) -> Result<(), String> {
        self.display_mode_kind()?;
        self.aspect_ratio_kind()?;
        self.capture_method_kind()?;
        Ok(())
    }

    pub fn capture_method_kind(&self) -> Result<CaptureMethod, String> {
        CaptureMethod::from_config_key(&self.capture_method).ok_or_else(|| {
            format!(
                "unsupported capture method {:?}; supported methods are portal and fixture",
                self.capture_method
            )
        })
    }

    pub fn display_mode_kind(&self) -> Result<DisplayMode, String> {
        DisplayMode::from_config_key(&self.display_mode).ok_or_else(|| {
            format!(
                "unsupported display mode {:?}; only borderless_fullscreen is supported",
                self.display_mode
            )
        })
    }

    pub fn aspect_ratio_kind(&self) -> Result<AspectRatio, String> {
        AspectRatio::from_config_key(&self.aspect_ratio).ok_or_else(|| {
            format!(
                "unsupported aspect ratio {:?}; only 16:9 is supported",
                self.aspect_ratio
            )
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMethod {
    Portal,
    Fixture,
}

impl CaptureMethod {
    pub fn from_config_key(value: &str) -> Option<Self> {
        match normalize_config_key(value).as_str() {
            "portal" => Some(Self::Portal),
            "fixture" => Some(Self::Fixture),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    BorderlessFullscreen,
}

impl DisplayMode {
    pub fn from_config_key(value: &str) -> Option<Self> {
        match normalize_config_key(value).as_str() {
            "borderlessfullscreen" => Some(Self::BorderlessFullscreen),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AspectRatio {
    SixteenByNine,
}

impl AspectRatio {
    pub fn from_config_key(value: &str) -> Option<Self> {
        match value.trim() {
            "16:9" => Some(Self::SixteenByNine),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ScannerConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_auto_delay_ms")]
    pub auto_delay_ms: u64,
    #[serde(default)]
    pub debug_images: bool,
    #[serde(default = "default_debug_image_retention_hours")]
    pub debug_image_retention_hours: u64,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_delay_ms: default_auto_delay_ms(),
            debug_images: false,
            debug_image_retention_hours: default_debug_image_retention_hours(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct HotkeyConfig {
    #[serde(default = "default_activation_hotkey")]
    pub activation: String,
    #[serde(default = "default_dismiss_overlay_hotkey")]
    pub dismiss_overlay: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            activation: default_activation_hotkey(),
            dismiss_overlay: default_dismiss_overlay_hotkey(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct OverlayConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub x_offset: i32,
    #[serde(default)]
    pub y_offset: i32,
    #[serde(default = "default_overlay_duration_ms")]
    pub duration_ms: u64,
    #[serde(default)]
    pub high_contrast: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            x_offset: 0,
            y_offset: 0,
            duration_ms: default_overlay_duration_ms(),
            high_contrast: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ClipboardConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub include_vaulted_marker: bool,
    #[serde(default)]
    pub footer: String,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            include_vaulted_marker: true,
            footer: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OcrConfig {
    #[serde(default = "default_ocr_language")]
    pub language: String,
    #[serde(default)]
    pub tesseract_data_path: String,
    #[serde(default)]
    pub confidence_threshold: f32,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            language: default_ocr_language(),
            tesseract_data_path: String::new(),
            confidence_threshold: 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct WarframeConfig {
    #[serde(default)]
    pub log_path: String,
    #[serde(default = "default_ui_theme")]
    pub ui_theme: String,
}

impl Default for WarframeConfig {
    fn default() -> Self {
        Self {
            log_path: String::new(),
            ui_theme: default_ui_theme(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: String::new(),
        }
    }
}

fn config_path(app_name: &str) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(app_name)
        .join("config.toml")
}

fn default_locale() -> String {
    "en".to_owned()
}

fn default_monitor() -> String {
    "primary".to_owned()
}

fn default_capture_method() -> String {
    "portal".to_owned()
}

fn default_display_mode() -> String {
    "borderless_fullscreen".to_owned()
}

fn default_aspect_ratio() -> String {
    "16:9".to_owned()
}

const fn default_true() -> bool {
    true
}

const fn default_auto_delay_ms() -> u64 {
    250
}

const fn default_debug_image_retention_hours() -> u64 {
    12
}

fn default_activation_hotkey() -> String {
    "F12".to_owned()
}

fn default_dismiss_overlay_hotkey() -> String {
    "F11".to_owned()
}

const fn default_overlay_duration_ms() -> u64 {
    10_000
}

fn default_ocr_language() -> String {
    "eng".to_owned()
}

fn default_ui_theme() -> String {
    "lotus".to_owned()
}

fn default_log_level() -> String {
    "info".to_owned()
}

fn normalize_config_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CaptureMethod, CliConfigOverrides, Config, Settings};
    use std::fs;

    #[test]
    fn partial_config_sections_use_defaults() {
        let config = toml::from_str::<Config>(
            r#"
[app]
locale = "nl"

[capture]
monitor = "DP-1"
"#,
        )
        .expect("partial config should deserialize");

        let settings = Settings::from(config);

        assert_eq!(settings.app.locale, "nl");
        assert!(!settings.app.start_minimized);
        assert_eq!(settings.capture.monitor, "DP-1");
        assert_eq!(settings.capture.capture_method, "portal");
        assert_eq!(settings.capture.display_mode, "borderless_fullscreen");
        assert_eq!(settings.capture.aspect_ratio, "16:9");
        assert!(settings.scanner.enabled);
        assert_eq!(settings.hotkeys.activation, "F12");
        assert_eq!(settings.hotkeys.dismiss_overlay, "F11");
        assert_eq!(settings.overlay.duration_ms, 10_000);
        assert!(!settings.clipboard.enabled);
        assert!(settings.clipboard.include_vaulted_marker);
        assert_eq!(settings.ocr.language, "eng");
        assert_eq!(settings.warframe.ui_theme, "lotus");
        assert_eq!(settings.logging.level, "info");
    }

    #[test]
    fn settings_convert_to_complete_config() {
        let mut settings = Settings::default();
        settings.set_capture_monitor("DP-1");
        settings.set_capture_portal_restore_token("token");

        let contents = toml::to_string_pretty(&settings).expect("settings should serialize");

        assert!(contents.contains("[app]"));
        assert!(contents.contains("[capture]"));
        assert!(contents.contains("[scanner]"));
        assert!(contents.contains("[hotkeys]"));
        assert!(contents.contains("[overlay]"));
        assert!(contents.contains("[clipboard]"));
        assert!(contents.contains("[ocr]"));
        assert!(contents.contains("[warframe]"));
        assert!(contents.contains("[logging]"));
        assert!(contents.contains("portal_restore_token = \"token\""));
    }

    #[test]
    fn settings_update_capture_restore_token_through_capture_api() {
        let mut settings = Settings::default();

        assert_eq!(settings.capture_portal_restore_token(), None);

        settings.set_capture_portal_restore_token("restored-session");

        assert_eq!(
            settings.capture_portal_restore_token(),
            Some("restored-session")
        );
        assert_eq!(
            settings.capture.portal_restore_token.as_deref(),
            Some("restored-session")
        );
    }

    #[test]
    fn read_or_create_writes_default_config_when_missing() {
        let path =
            std::env::temp_dir().join(format!("wf-info-config-create-{}.toml", std::process::id()));
        let _ = fs::remove_file(&path);

        let config = Config::read_or_create(&path).expect("missing config should be created");

        assert_eq!(config, Config::default());
        assert!(path.exists());
        let contents = fs::read_to_string(&path).expect("created config should be readable");
        assert!(contents.contains("[warframe]"));
        assert!(contents.contains("ui_theme = \"lotus\""));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn cli_overrides_replace_loaded_logging_level_without_changing_other_config() {
        let config = toml::from_str::<Config>(
            r#"
[app]
locale = "nl"

[logging]
level = "warn"
file = "/tmp/wf-info.log"
"#,
        )
        .expect("config should deserialize");
        let mut overrides = CliConfigOverrides::default();
        overrides.set_logging_level("debug");

        let config = config.with_cli_overrides(&overrides);

        assert_eq!(overrides.logging_level(), Some("debug"));
        assert_eq!(config.app.locale, "nl");
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.file, "/tmp/wf-info.log");
    }

    #[test]
    fn capture_config_exposes_supported_typed_values() {
        let mut settings = Settings::default();
        settings.capture.capture_method = " Fixture ".to_owned();
        settings.capture.display_mode = "borderless fullscreen".to_owned();

        settings
            .capture
            .validate_supported()
            .expect("supported capture config");

        assert_eq!(
            settings.capture.capture_method_kind(),
            Ok(CaptureMethod::Fixture)
        );
    }

    #[test]
    fn capture_config_rejects_unsupported_values() {
        let mut settings = Settings::default();
        settings.capture.display_mode = "windowed".to_owned();

        let err = settings
            .capture
            .validate_supported()
            .expect_err("unsupported mode");

        assert!(err.contains("borderless_fullscreen"));
    }
}
