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
    pub overlay: OverlayConfig,
    #[serde(default)]
    pub ocr: OcrConfig,
    #[serde(default)]
    pub warframe: WarframeConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Config {
    pub fn for_app(app_name: &str) -> Result<Self, String> {
        Self::read(Self::path_for_app(app_name))
    }

    pub fn with_cli_overrides(mut self, overrides: &CliConfigOverrides) -> Self {
        self.apply_cli_overrides(overrides);
        self
    }

    pub fn apply_cli_overrides(&mut self, overrides: &CliConfigOverrides) {
        self.logging.apply_cli_overrides(&overrides.logging);
    }

    pub fn from_settings(settings: &Settings) -> Self {
        Self::from(settings)
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

    pub fn write_for_app(&self, app_name: &str) -> Result<(), String> {
        self.write(Self::path_for_app(app_name))
    }
}

impl From<&Settings> for Config {
    fn from(settings: &Settings) -> Self {
        Self {
            app: AppConfig::from(&settings.app),
            capture: CaptureConfig::from(&settings.capture),
            scanner: ScannerConfig::from(&settings.scanner),
            overlay: OverlayConfig::from(&settings.overlay),
            ocr: OcrConfig::from(&settings.ocr),
            warframe: WarframeConfig::from(&settings.warframe),
            logging: LoggingConfig::from(&settings.logging),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub app: AppSettings,
    pub capture: CaptureSettings,
    pub scanner: ScannerSettings,
    pub overlay: OverlaySettings,
    pub ocr: OcrSettings,
    pub warframe: WarframeSettings,
    pub logging: LoggingSettings,
}

impl Settings {
    pub fn apply_cli_overrides(&mut self, overrides: &CliConfigOverrides) {
        self.logging.apply_cli_overrides(&overrides.logging);
    }

    pub fn to_config(&self) -> Config {
        Config::from(self)
    }

    pub fn capture_portal_restore_token(&self) -> Option<&str> {
        self.capture.portal_restore_token()
    }

    pub fn set_capture_monitor(&mut self, monitor: impl Into<String>) {
        self.capture.set_monitor(monitor);
    }

    pub fn set_capture_portal_restore_token(&mut self, restore_token: impl Into<String>) {
        self.capture.set_portal_restore_token(restore_token);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliConfigOverrides {
    logging: CliLoggingOverrides,
}

impl CliConfigOverrides {
    pub fn set_logging_level(&mut self, level: impl Into<String>) {
        self.logging.set_level(level);
    }

    pub fn logging_level(&self) -> Option<&str> {
        self.logging.level()
    }

    pub fn is_empty(&self) -> bool {
        self.logging.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CliLoggingOverrides {
    level: Option<String>,
}

impl CliLoggingOverrides {
    fn set_level(&mut self, level: impl Into<String>) {
        self.level = Some(level.into());
    }

    fn level(&self) -> Option<&str> {
        self.level.as_deref()
    }

    fn is_empty(&self) -> bool {
        self.level.is_none()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::from(Config::default())
    }
}

impl From<Config> for Settings {
    fn from(config: Config) -> Self {
        Self {
            app: AppSettings::from(config.app),
            capture: CaptureSettings::from(config.capture),
            scanner: ScannerSettings::from(config.scanner),
            overlay: OverlaySettings::from(config.overlay),
            ocr: OcrSettings::from(config.ocr),
            warframe: WarframeSettings::from(config.warframe),
            logging: LoggingSettings::from(config.logging),
        }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppSettings {
    pub locale: String,
    pub start_minimized: bool,
}

impl From<AppConfig> for AppSettings {
    fn from(config: AppConfig) -> Self {
        Self {
            locale: config.locale,
            start_minimized: config.start_minimized,
        }
    }
}

impl From<&AppSettings> for AppConfig {
    fn from(settings: &AppSettings) -> Self {
        Self {
            locale: settings.locale.clone(),
            start_minimized: settings.start_minimized,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureSettings {
    pub monitor: String,
    pub capture_method: String,
    pub display_mode: String,
    pub aspect_ratio: String,
    pub portal_restore_token: Option<String>,
}

impl CaptureSettings {
    pub fn portal_restore_token(&self) -> Option<&str> {
        self.portal_restore_token.as_deref()
    }

    pub fn set_monitor(&mut self, monitor: impl Into<String>) {
        self.monitor = monitor.into();
    }

    pub fn set_portal_restore_token(&mut self, restore_token: impl Into<String>) {
        self.portal_restore_token = Some(restore_token.into());
    }
}

impl From<CaptureConfig> for CaptureSettings {
    fn from(config: CaptureConfig) -> Self {
        Self {
            monitor: config.monitor,
            capture_method: config.capture_method,
            display_mode: config.display_mode,
            aspect_ratio: config.aspect_ratio,
            portal_restore_token: config.portal_restore_token,
        }
    }
}

impl From<&CaptureSettings> for CaptureConfig {
    fn from(settings: &CaptureSettings) -> Self {
        Self {
            monitor: settings.monitor.clone(),
            capture_method: settings.capture_method.clone(),
            display_mode: settings.display_mode.clone(),
            aspect_ratio: settings.aspect_ratio.clone(),
            portal_restore_token: settings.portal_restore_token.clone(),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannerSettings {
    pub enabled: bool,
    pub auto_delay_ms: u64,
    pub debug_images: bool,
    pub debug_image_retention_hours: u64,
}

impl From<ScannerConfig> for ScannerSettings {
    fn from(config: ScannerConfig) -> Self {
        Self {
            enabled: config.enabled,
            auto_delay_ms: config.auto_delay_ms,
            debug_images: config.debug_images,
            debug_image_retention_hours: config.debug_image_retention_hours,
        }
    }
}

impl From<&ScannerSettings> for ScannerConfig {
    fn from(settings: &ScannerSettings) -> Self {
        Self {
            enabled: settings.enabled,
            auto_delay_ms: settings.auto_delay_ms,
            debug_images: settings.debug_images,
            debug_image_retention_hours: settings.debug_image_retention_hours,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlaySettings {
    pub enabled: bool,
    pub x_offset: i32,
    pub y_offset: i32,
    pub duration_ms: u64,
    pub high_contrast: bool,
}

impl From<OverlayConfig> for OverlaySettings {
    fn from(config: OverlayConfig) -> Self {
        Self {
            enabled: config.enabled,
            x_offset: config.x_offset,
            y_offset: config.y_offset,
            duration_ms: config.duration_ms,
            high_contrast: config.high_contrast,
        }
    }
}

impl From<&OverlaySettings> for OverlayConfig {
    fn from(settings: &OverlaySettings) -> Self {
        Self {
            enabled: settings.enabled,
            x_offset: settings.x_offset,
            y_offset: settings.y_offset,
            duration_ms: settings.duration_ms,
            high_contrast: settings.high_contrast,
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

#[derive(Clone, Debug, PartialEq)]
pub struct OcrSettings {
    pub language: String,
    pub tesseract_data_path: String,
    pub confidence_threshold: f32,
}

impl From<OcrConfig> for OcrSettings {
    fn from(config: OcrConfig) -> Self {
        Self {
            language: config.language,
            tesseract_data_path: config.tesseract_data_path,
            confidence_threshold: config.confidence_threshold,
        }
    }
}

impl From<&OcrSettings> for OcrConfig {
    fn from(settings: &OcrSettings) -> Self {
        Self {
            language: settings.language.clone(),
            tesseract_data_path: settings.tesseract_data_path.clone(),
            confidence_threshold: settings.confidence_threshold,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarframeSettings {
    pub log_path: String,
    pub ui_theme: String,
}

impl From<WarframeConfig> for WarframeSettings {
    fn from(config: WarframeConfig) -> Self {
        Self {
            log_path: config.log_path,
            ui_theme: config.ui_theme,
        }
    }
}

impl From<&WarframeSettings> for WarframeConfig {
    fn from(settings: &WarframeSettings) -> Self {
        Self {
            log_path: settings.log_path.clone(),
            ui_theme: settings.ui_theme.clone(),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggingSettings {
    pub level: String,
    pub file: String,
}

impl From<LoggingConfig> for LoggingSettings {
    fn from(config: LoggingConfig) -> Self {
        Self {
            level: config.level,
            file: config.file,
        }
    }
}

impl From<&LoggingSettings> for LoggingConfig {
    fn from(settings: &LoggingSettings) -> Self {
        Self {
            level: settings.level.clone(),
            file: settings.file.clone(),
        }
    }
}

impl LoggingConfig {
    fn apply_cli_overrides(&mut self, overrides: &CliLoggingOverrides) {
        if let Some(level) = overrides.level() {
            self.set_level(level);
        }
    }

    fn set_level(&mut self, level: impl Into<String>) {
        self.level = level.into();
    }
}

impl LoggingSettings {
    fn apply_cli_overrides(&mut self, overrides: &CliLoggingOverrides) {
        if let Some(level) = overrides.level() {
            self.set_level(level);
        }
    }

    fn set_level(&mut self, level: impl Into<String>) {
        self.level = level.into();
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

#[cfg(test)]
mod tests {
    use super::{CliConfigOverrides, Config, Settings};

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
        assert_eq!(settings.overlay.duration_ms, 10_000);
        assert_eq!(settings.ocr.language, "eng");
        assert_eq!(settings.warframe.ui_theme, "lotus");
        assert_eq!(settings.logging.level, "info");
    }

    #[test]
    fn settings_convert_to_complete_config() {
        let mut settings = Settings::default();
        settings.set_capture_monitor("DP-1");
        settings.set_capture_portal_restore_token("token");

        let contents =
            toml::to_string_pretty(&settings.to_config()).expect("settings should serialize");

        assert!(contents.contains("[app]"));
        assert!(contents.contains("[capture]"));
        assert!(contents.contains("[scanner]"));
        assert!(contents.contains("[overlay]"));
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
            settings.to_config().capture.portal_restore_token.as_deref(),
            Some("restored-session")
        );
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
        let mut settings = Settings::from(config.clone());
        settings.apply_cli_overrides(&overrides);

        assert_eq!(overrides.logging_level(), Some("debug"));
        assert_eq!(config.app.locale, "nl");
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.file, "/tmp/wf-info.log");
        assert_eq!(settings.logging.level, "debug");
        assert_eq!(settings.logging.file, "/tmp/wf-info.log");
    }
}
