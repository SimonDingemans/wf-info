use shared::monitor;
use shared::rewards::RewardOverlayEntry;
use shared::watchers::events::ServiceEvent;

use crate::data_cache::DataCacheRefresh;

use super::monitor::MonitorChoice;

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
    ServiceEvent(ServiceEvent),
    RewardScanFinished(Result<Vec<RewardOverlayEntry>, String>),
}
