use std::{
    fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use ocr::implementations::reward_screen::{
    RewardUiTheme, scan_reward_screen_frame, scan_reward_screen_frame_with_debug_images,
};
use ocr::{
    CaptureProvider, CaptureRequest, CapturedFrame, OcrOptions, PortalScreenshotCaptureProvider,
    Rect,
};
use shared::config::{CaptureMethod, Settings as AppSettings};
use shared::monitor::{MonitorCaptureRegion, MonitorRegion};
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
    monitor_region: Option<MonitorCaptureRegion>,
) -> Result<Vec<RewardOverlayEntry>, String> {
    log::debug!("reward scan requested from {trigger:?}");

    let started = Instant::now();
    ensure_supported_capture_settings(&settings)?;
    let frame = capture_reward_frame(&settings, monitor_region)?;
    let capture_elapsed = started.elapsed();
    let debug_capture_path =
        write_debug_capture_if_enabled(&frame, &trigger, &settings, &debug_capture_dir);
    let theme = reward_ui_theme_from_settings(&settings)?;
    let options = ocr_options_from_settings(&settings);
    let ocr_started = Instant::now();
    let candidates = if debug_capture_output_enabled(&settings) {
        scan_reward_screen_frame_with_debug_images(&frame, theme, options, &debug_capture_dir)
    } else {
        scan_reward_screen_frame(&frame, theme, options)
    }
    .map_err(|err| err.to_string())?;
    let ocr_elapsed = ocr_started.elapsed();
    let rewards = candidates
        .into_iter()
        .filter_map(reward_candidate_to_overlay_entry)
        .collect::<Vec<_>>();

    log::info!(
        "reward scan produced {} overlay reward entrie(s) in {:?} (capture={:?}, ocr={:?}); debug_capture={:?}",
        rewards.len(),
        started.elapsed(),
        capture_elapsed,
        ocr_elapsed,
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

fn capture_reward_frame(
    settings: &AppSettings,
    monitor_region: Option<MonitorCaptureRegion>,
) -> Result<CapturedFrame, String> {
    match settings.capture.capture_method_kind()? {
        CaptureMethod::Fixture => {
            log::debug!("using bundled reward screen fixture as configured capture source");
            ocr::debug::reward_screen_fixture_frame().map_err(|err| err.to_string())
        }
        CaptureMethod::Portal => {
            let provider = PortalScreenshotCaptureProvider;
            let request = capture_request_from_settings(settings, monitor_region)?;

            provider.capture(request).map_err(|err| err.to_string())
        }
    }
}

fn capture_request_from_settings(
    settings: &AppSettings,
    monitor_region: Option<MonitorCaptureRegion>,
) -> Result<CaptureRequest, String> {
    let mut request = CaptureRequest::new(settings.capture.monitor.clone());

    if let Some((region, desktop_bounds)) = configured_monitor_region(settings, monitor_region)? {
        request = request.with_region(region, desktop_bounds);
    }

    Ok(request)
}

fn configured_monitor_region(
    settings: &AppSettings,
    monitor_region: Option<MonitorCaptureRegion>,
) -> Result<Option<(Rect, Rect)>, String> {
    if let Some(region) = monitor_region {
        log::debug!(
            "using cached selected monitor capture region {:?} within desktop {:?}",
            region.region,
            region.desktop_bounds
        );
        return Ok(Some(ocr_capture_region(region)));
    }

    let target = settings.capture.monitor.trim();
    if shared::monitor::target_uses_full_screenshot(target) {
        log::debug!(
            "capture monitor is {:?}; using full portal screenshot without monitor crop",
            settings.capture.monitor
        );
        return Ok(None);
    }

    let monitors = shared::monitor::detect_monitor_info()?;
    let Some(region) = shared::monitor::capture_region_for_target(&monitors, target)? else {
        return Ok(None);
    };

    log::debug!(
        "configured capture monitor {:?} resolved to logical crop {:?} within logical desktop {:?}",
        settings.capture.monitor,
        region.region,
        region.desktop_bounds
    );

    Ok(Some(ocr_capture_region(region)))
}

fn ocr_capture_region(region: MonitorCaptureRegion) -> (Rect, Rect) {
    (ocr_rect(region.region), ocr_rect(region.desktop_bounds))
}

fn ocr_rect(region: MonitorRegion) -> Rect {
    Rect {
        x: region.x,
        y: region.y,
        width: region.width,
        height: region.height,
    }
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
        debug_capture_output_enabled, ensure_supported_capture_settings, ocr_options_from_settings,
        reward_candidate_to_overlay_entry, reward_ui_theme_from_settings,
    };
    use ocr::implementations::reward_screen::RewardNameCandidate;
    use ocr::implementations::reward_screen::RewardUiTheme;
    use ocr::{Rect, ScanRegion};
    use shared::config::Settings;

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
    fn debug_capture_output_is_enabled_by_scanner_setting() {
        let mut settings = Settings::default();

        assert!(!debug_capture_output_enabled(&settings));

        settings.scanner.debug_images = true;
        assert!(debug_capture_output_enabled(&settings));

        settings.scanner.debug_images = false;
        settings.logging.level = " debug ".to_owned();
        assert!(!debug_capture_output_enabled(&settings));
    }

    #[test]
    fn reward_ui_theme_uses_configured_warframe_theme() {
        let mut settings = Settings::default();
        settings.warframe.ui_theme = "vitruvian".to_owned();

        let theme = reward_ui_theme_from_settings(&settings).expect("known theme");

        assert_eq!(theme, RewardUiTheme::Vitruvian);
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
