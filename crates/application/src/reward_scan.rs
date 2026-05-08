use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ocr::implementations::reward_screen::{
    RewardUiTheme, scan_reward_screen_frame, scan_reward_screen_frame_with_debug_images,
};
use ocr::{
    CaptureProvider, CaptureRequest, CapturedFrame, OcrOptions, PortalScreenshotCaptureProvider,
    Rect,
};
use shared::config::{CaptureMethod, Settings as AppSettings};
use shared::monitor::MonitorInfo;
use shared::rewards::RewardOverlayEntry;
use shared::watchers::log_watcher::RewardScreenDetection;

#[derive(Clone, Debug)]
pub(crate) enum RewardScanTrigger {
    Log(RewardScreenDetection),
    Hotkey(String),
}

pub(crate) async fn scan_rewards_for_overlay(
    trigger: RewardScanTrigger,
    settings: AppSettings,
    debug_capture_dir: PathBuf,
) -> Result<Vec<RewardOverlayEntry>, String> {
    log::debug!("reward scan requested from {trigger:?}");

    ensure_supported_capture_settings(&settings)?;
    let frame = capture_reward_frame(&settings)?;
    let debug_capture_path =
        write_debug_capture_if_enabled(&frame, &trigger, &settings, &debug_capture_dir);
    let theme = reward_ui_theme_from_settings(&settings)?;
    let options = ocr_options_from_settings(&settings);
    let candidates = if debug_capture_output_enabled(&settings) {
        scan_reward_screen_frame_with_debug_images(&frame, theme, options, &debug_capture_dir)
    } else {
        scan_reward_screen_frame(&frame, theme, options)
    }
    .map_err(|err| err.to_string())?;
    let rewards = candidates
        .into_iter()
        .filter_map(reward_candidate_to_overlay_entry)
        .collect::<Vec<_>>();

    log::debug!(
        "reward scan produced {} overlay reward entrie(s); debug_capture={:?}",
        rewards.len(),
        debug_capture_path
    );

    Ok(rewards)
}

fn write_debug_capture_if_enabled(
    frame: &CapturedFrame,
    trigger: &RewardScanTrigger,
    settings: &AppSettings,
    directory: &Path,
) -> Option<PathBuf> {
    if !debug_capture_output_enabled(settings) {
        return None;
    }

    match write_debug_capture(frame, trigger, directory) {
        Ok(path) => {
            log::debug!("saved reward scan debug capture to {}", path.display());
            Some(path)
        }
        Err(err) => {
            log::warn!("failed to save reward scan debug capture: {err}");
            None
        }
    }
}

fn write_debug_capture(
    frame: &CapturedFrame,
    trigger: &RewardScanTrigger,
    directory: &Path,
) -> Result<PathBuf, String> {
    fs::create_dir_all(directory).map_err(|err| err.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_millis();
    let filename = format!(
        "reward-capture-{timestamp}-{}.png",
        debug_capture_trigger_label(trigger)
    );
    let path = directory.join(filename);
    frame.image().save(&path).map_err(|err| err.to_string())?;

    let latest_path = directory.join("latest-reward-capture.png");
    frame
        .image()
        .save(&latest_path)
        .map_err(|err| err.to_string())?;
    let latest_ocr_frame_path = directory.join("latest-ocr-frame.png");
    frame
        .image()
        .save(&latest_ocr_frame_path)
        .map_err(|err| err.to_string())?;

    Ok(path)
}

fn debug_capture_output_enabled(settings: &AppSettings) -> bool {
    settings.scanner.debug_images
        || matches!(
            settings.logging.level.trim().to_ascii_lowercase().as_str(),
            "debug" | "trace"
        )
}

fn debug_capture_trigger_label(trigger: &RewardScanTrigger) -> &'static str {
    match trigger {
        RewardScanTrigger::Log(_) => "log",
        RewardScanTrigger::Hotkey(_) => "hotkey",
    }
}

fn ensure_supported_capture_settings(settings: &AppSettings) -> Result<(), String> {
    settings.capture.validate_supported()
}

fn capture_reward_frame(settings: &AppSettings) -> Result<CapturedFrame, String> {
    match settings.capture.capture_method_kind()? {
        CaptureMethod::Fixture => {
            log::debug!("using bundled reward screen fixture as configured capture source");
            ocr::debug::reward_screen_fixture_frame().map_err(|err| err.to_string())
        }
        CaptureMethod::Portal => {
            let provider = PortalScreenshotCaptureProvider;
            let request = capture_request_from_settings(settings)?;

            provider.capture(request).map_err(|err| err.to_string())
        }
    }
}

fn capture_request_from_settings(settings: &AppSettings) -> Result<CaptureRequest, String> {
    let mut request = CaptureRequest::new(settings.capture.monitor.clone());

    if let Some((region, desktop_bounds)) = configured_monitor_region(settings)? {
        request = request.with_region(region, desktop_bounds);
    }

    Ok(request)
}

fn configured_monitor_region(settings: &AppSettings) -> Result<Option<(Rect, Rect)>, String> {
    let target = settings.capture.monitor.trim();
    if target.is_empty() || target.eq_ignore_ascii_case("primary") {
        log::debug!(
            "capture monitor is {:?}; using full portal screenshot without monitor crop",
            settings.capture.monitor
        );
        return Ok(None);
    }

    let monitors = shared::monitor::detect_monitor_info()?;
    let Some(monitor) = find_configured_monitor(&monitors, target) else {
        let available = monitors
            .iter()
            .map(MonitorInfo::display_name)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "configured capture monitor {:?} was not found; available monitors: {}",
            settings.capture.monitor,
            if available.is_empty() {
                "none".to_owned()
            } else {
                available
            }
        ));
    };

    let region = monitor_region(monitor)?;
    let desktop_bounds = desktop_bounds(&monitors)?;
    log::debug!(
        "configured capture monitor {:?} resolved to logical crop {:?} within logical desktop {:?}",
        settings.capture.monitor,
        region,
        desktop_bounds
    );

    Ok(Some((region, desktop_bounds)))
}

fn find_configured_monitor<'a>(
    monitors: &'a [MonitorInfo],
    target: &str,
) -> Option<&'a MonitorInfo> {
    monitors
        .iter()
        .find(|monitor| monitor_matches(monitor, target))
}

fn monitor_matches(monitor: &MonitorInfo, target: &str) -> bool {
    let normalized_target = normalize_monitor_target(target);
    [
        monitor.xdg_output_name.as_deref(),
        monitor.id.as_deref(),
        monitor.mapping_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| normalize_monitor_target(value) == normalized_target)
}

fn normalize_monitor_target(target: &str) -> String {
    target.trim().to_ascii_lowercase()
}

fn monitor_region(monitor: &MonitorInfo) -> Result<Rect, String> {
    let (x, y) = monitor.position.ok_or_else(|| {
        format!(
            "capture monitor {} does not have a known position",
            monitor.display_name()
        )
    })?;
    let (width, height) = monitor.size.ok_or_else(|| {
        format!(
            "capture monitor {} does not have a known size",
            monitor.display_name()
        )
    })?;

    if x < 0 || y < 0 || width <= 0 || height <= 0 {
        return Err(format!(
            "capture monitor {} has unsupported geometry: position={:?}, size={:?}",
            monitor.display_name(),
            monitor.position,
            monitor.size
        ));
    }

    Ok(Rect {
        x: x as u32,
        y: y as u32,
        width: width as u32,
        height: height as u32,
    })
}

fn desktop_bounds(monitors: &[MonitorInfo]) -> Result<Rect, String> {
    let mut regions = monitors
        .iter()
        .map(monitor_region)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let Some(first) = regions.next() else {
        return Err("no monitor geometry was available for capture scaling".to_owned());
    };

    let bounds = regions.fold(first, |bounds, region| Rect {
        x: bounds.x.min(region.x),
        y: bounds.y.min(region.y),
        width: bounds.right().max(region.right()) - bounds.x.min(region.x),
        height: bounds.bottom().max(region.bottom()) - bounds.y.min(region.y),
    });

    log::debug!("logical desktop bounds for capture scaling: {bounds:?}");

    Ok(bounds)
}

fn reward_ui_theme_from_settings(settings: &AppSettings) -> Result<RewardUiTheme, String> {
    RewardUiTheme::from_config_key(&settings.warframe.ui_theme).ok_or_else(|| {
        format!(
            "unsupported Warframe UI theme {:?}",
            settings.warframe.ui_theme
        )
    })
}

fn ocr_options_from_settings(settings: &AppSettings) -> OcrOptions {
    OcrOptions {
        language: settings.ocr.language.clone(),
        tesseract_data_path: (!settings.ocr.tesseract_data_path.trim().is_empty())
            .then(|| settings.ocr.tesseract_data_path.clone()),
        confidence_threshold: settings.ocr.confidence_threshold,
    }
}

fn reward_candidate_to_overlay_entry(
    candidate: ocr::implementations::reward_screen::RewardNameCandidate,
) -> Option<RewardOverlayEntry> {
    let name = candidate
        .raw_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if name.is_empty() {
        return None;
    }

    Some(RewardOverlayEntry::name_only(name))
}

#[cfg(test)]
mod tests {
    use super::{
        debug_capture_output_enabled, desktop_bounds, ensure_supported_capture_settings,
        ocr_options_from_settings, reward_candidate_to_overlay_entry,
        reward_ui_theme_from_settings,
    };
    use ocr::implementations::reward_screen::RewardNameCandidate;
    use ocr::implementations::reward_screen::RewardUiTheme;
    use ocr::{Rect, ScanRegion};
    use shared::config::Settings;
    use shared::monitor::MonitorInfo;

    #[test]
    fn ocr_options_use_configured_settings_and_ignore_empty_tessdata_path() {
        let mut settings = Settings::default();
        settings.ocr.language = "fra".to_owned();
        settings.ocr.tesseract_data_path = "  ".to_owned();
        settings.ocr.confidence_threshold = 42.0;

        let options = ocr_options_from_settings(&settings);

        assert_eq!(options.language, "fra");
        assert_eq!(options.tesseract_data_path, None);
        assert_eq!(options.confidence_threshold, 42.0);
    }

    #[test]
    fn capture_settings_reject_unsupported_display_modes() {
        let mut settings = Settings::default();
        settings.capture.display_mode = "windowed".to_owned();

        let err = ensure_supported_capture_settings(&settings).expect_err("unsupported mode");

        assert!(err.contains("borderless_fullscreen"));
    }

    #[test]
    fn capture_method_is_case_and_whitespace_insensitive() {
        let mut settings = Settings::default();
        settings.capture.capture_method = " Portal ".to_owned();

        ensure_supported_capture_settings(&settings).expect("portal method");
    }

    #[test]
    fn debug_capture_output_is_enabled_by_scanner_setting_or_debug_logging() {
        let mut settings = Settings::default();

        assert!(!debug_capture_output_enabled(&settings));

        settings.scanner.debug_images = true;
        assert!(debug_capture_output_enabled(&settings));

        settings.scanner.debug_images = false;
        settings.logging.level = " debug ".to_owned();
        assert!(debug_capture_output_enabled(&settings));
    }

    #[test]
    fn reward_ui_theme_uses_configured_warframe_theme() {
        let mut settings = Settings::default();
        settings.warframe.ui_theme = "vitruvian".to_owned();

        let theme = reward_ui_theme_from_settings(&settings).expect("known theme");

        assert_eq!(theme, RewardUiTheme::Vitruvian);
    }

    #[test]
    fn configured_monitor_geometry_becomes_capture_region() {
        let monitor = MonitorInfo {
            pipe_wire_node_id: None,
            id: Some("DP-3".to_owned()),
            mapping_id: None,
            position: Some((2649, 0)),
            size: Some((2648, 1490)),
            source_type: Some("Wayland xdg-output".to_owned()),
            xdg_output_name: Some("DP-3".to_owned()),
        };

        let region = super::monitor_region(&monitor).expect("monitor region");

        assert_eq!(
            region,
            Rect {
                x: 2649,
                y: 0,
                width: 2648,
                height: 1490
            }
        );
    }

    #[test]
    fn configured_monitor_matches_wayland_output_name_case_insensitively() {
        let monitor = MonitorInfo {
            pipe_wire_node_id: None,
            id: Some("other".to_owned()),
            mapping_id: None,
            position: Some((0, 0)),
            size: Some((1920, 1080)),
            source_type: None,
            xdg_output_name: Some("DP-1".to_owned()),
        };

        assert!(super::monitor_matches(&monitor, "dp-1"));
    }

    #[test]
    fn desktop_bounds_cover_all_monitor_logical_regions() {
        let monitors = vec![
            MonitorInfo {
                pipe_wire_node_id: None,
                id: Some("DP-3".to_owned()),
                mapping_id: None,
                position: Some((0, 0)),
                size: Some((2648, 1490)),
                source_type: Some("Wayland xdg-output".to_owned()),
                xdg_output_name: Some("DP-3".to_owned()),
            },
            MonitorInfo {
                pipe_wire_node_id: None,
                id: Some("DP-1".to_owned()),
                mapping_id: None,
                position: Some((2649, 0)),
                size: Some((2648, 1490)),
                source_type: Some("Wayland xdg-output".to_owned()),
                xdg_output_name: Some("DP-1".to_owned()),
            },
        ];

        let bounds = desktop_bounds(&monitors).expect("desktop bounds");

        assert_eq!(
            bounds,
            Rect {
                x: 0,
                y: 0,
                width: 5297,
                height: 1490
            }
        );
    }

    #[test]
    fn reward_candidate_text_is_normalized_for_overlay_display() {
        let candidate = RewardNameCandidate {
            raw_text: " Forma\n Blueprint ".to_owned(),
            normalized_text: "forma blueprint".to_owned(),
            confidence: Some(96.0),
            region: ScanRegion::new(
                "reward-1",
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 24,
                },
            ),
        };

        let entry = reward_candidate_to_overlay_entry(candidate).expect("reward entry");

        assert_eq!(entry.name, "Forma Blueprint");
    }
}
