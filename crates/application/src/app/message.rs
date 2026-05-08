use shared::monitor;
use shared::rewards::RewardOverlayEntry;
use shared::watchers::events::ServiceEvent;

use crate::data_cache::DataCacheRefresh;

use super::monitor::MonitorChoice;
use super::settings::SettingsTab;

#[derive(Clone, Debug)]
pub(super) enum Message {
    DetectMonitorInfo,
    StartupMonitorInfoDetected(Result<Vec<monitor::MonitorInfo>, String>),
    MonitorInfoDetected(Result<Vec<monitor::MonitorInfo>, String>),
    SelectedMonitorChanged(MonitorChoice),
    DrawTestOverlay,
    TestOverlayLaunched(Result<u32, String>),
    QuitDebugOverlays,
    RefreshDataCache,
    DataCacheRefreshFinished(Result<DataCacheRefresh, String>),
    ClipboardOutputChanged(bool),
    OpenSettings,
    CancelSettings,
    SaveSettings,
    SettingsTabSelected(SettingsTab),
    SettingsAppLocaleChanged(String),
    SettingsAppStartMinimizedChanged(bool),
    SettingsCaptureMonitorChanged(String),
    SettingsCaptureMethodChanged(String),
    SettingsDisplayModeChanged(String),
    SettingsAspectRatioChanged(String),
    SettingsScannerEnabledChanged(bool),
    SettingsScannerAutoDelayChanged(String),
    SettingsScannerDebugImagesChanged(bool),
    SettingsScannerRetentionChanged(String),
    SettingsActivationHotkeyChanged(String),
    SettingsDismissOverlayHotkeyChanged(String),
    SettingsOverlayEnabledChanged(bool),
    SettingsOverlayXOffsetChanged(String),
    SettingsOverlayYOffsetChanged(String),
    SettingsOverlayDurationChanged(String),
    SettingsOverlayHighContrastChanged(bool),
    SettingsClipboardEnabledChanged(bool),
    SettingsClipboardVaultedMarkerChanged(bool),
    SettingsClipboardFooterChanged(String),
    SettingsOcrLanguageChanged(String),
    SettingsTesseractDataPathChanged(String),
    SettingsOcrConfidenceChanged(String),
    SettingsWarframeLogPathChanged(String),
    SettingsWarframeUiThemeChanged(String),
    SettingsLoggingLevelChanged(String),
    SettingsLoggingFileChanged(String),
    ServiceEvent(ServiceEvent),
    RewardScanFinished(Result<Vec<RewardOverlayEntry>, String>),
}
