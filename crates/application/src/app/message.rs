use shared::monitor;
use shared::rewards::RewardOverlayEntry;
use shared::watchers::events::ServiceEvent;

use crate::data_cache::DataCacheRefresh;

use super::monitor::MonitorChoice;
use super::settings::{HotkeyCaptureTarget, SettingsTab};

#[derive(Clone, Debug)]
pub(super) enum Message {
    DetectMonitorInfo,
    StartupMonitorInfoDetected(Result<Vec<monitor::MonitorInfo>, String>),
    MonitorInfoDetected(Result<Vec<monitor::MonitorInfo>, String>),
    SelectedMonitorChanged(MonitorChoice),
    DrawTestOverlay,
    TestOverlayLaunched(Result<u32, String>),
    QuitDebugOverlays,
    ToggleDiagnostics,
    ScanNow,
    RefreshDataCache,
    DataCacheRefreshFinished(Result<DataCacheRefresh, String>),
    ClipboardOutputChanged(bool),
    OpenSettings,
    CancelSettings,
    SaveSettings,
    SettingsTabSelected(SettingsTab),
    UiEvent(iced::Event),
    SettingsAppLocaleChanged(String),
    SettingsAppStartMinimizedChanged(bool),
    SettingsCaptureMonitorChanged(String),
    SettingsCaptureMethodChanged(String),
    SettingsDisplayModeChanged(String),
    SettingsAspectRatioChanged(String),
    SettingsScannerEnabledChanged(bool),
    SettingsScannerAutoDelayChanged(u32),
    SettingsScannerDebugImagesChanged(bool),
    SettingsScannerRetentionChanged(u32),
    StartHotkeyCapture(HotkeyCaptureTarget),
    SettingsOverlayEnabledChanged(bool),
    SettingsOverlayXOffsetChanged(i32),
    SettingsOverlayYOffsetChanged(i32),
    SettingsOverlayDurationChanged(u32),
    SettingsOverlayHighContrastChanged(bool),
    SettingsClipboardEnabledChanged(bool),
    SettingsClipboardVaultedMarkerChanged(bool),
    SettingsClipboardFooterChanged(String),
    SettingsOcrLanguageChanged(String),
    SettingsTesseractDataPathChanged(String),
    SettingsOcrConfidenceChanged(f32),
    SettingsWarframeLogPathChanged(String),
    SettingsWarframeUiThemeChanged(String),
    SettingsLoggingLevelChanged(String),
    SettingsLoggingFileChanged(String),
    ServiceEvent(ServiceEvent),
    RewardScanFinished(Result<Vec<RewardOverlayEntry>, String>),
}
