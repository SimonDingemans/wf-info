use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::implementations::reward_screen::{RewardNameCandidate, RewardScreenScanner};
use crate::tesseract::TesseractRecognizer;
use crate::{CapturedFrame, FeatureScanner, OcrOptions, Result};

use super::RewardUiTheme;

pub fn scan_reward_screen_frame(
    frame: &CapturedFrame,
    theme: RewardUiTheme,
    options: OcrOptions,
) -> Result<Vec<RewardNameCandidate>> {
    log::debug!(
        "running reward screen OCR on captured frame: {}x{}, theme={theme}",
        frame.width(),
        frame.height()
    );

    scan_with_cached_scanner(frame, theme, options)
}

pub fn scan_reward_screen_frame_with_debug_images(
    frame: &CapturedFrame,
    theme: RewardUiTheme,
    options: OcrOptions,
    debug_dir: &Path,
) -> Result<Vec<RewardNameCandidate>> {
    log::debug!(
        "running reward screen OCR on captured frame with debug images: {}x{}, theme={}, debug_dir={}",
        frame.width(),
        frame.height(),
        theme,
        debug_dir.display()
    );

    let recognizer = TesseractRecognizer::new(&options)?;
    let scanner = RewardScreenScanner::new(recognizer, theme, options).with_debug_images(debug_dir);

    scanner.scan(frame)
}

fn scan_with_cached_scanner(
    frame: &CapturedFrame,
    theme: RewardUiTheme,
    options: OcrOptions,
) -> Result<Vec<RewardNameCandidate>> {
    let mut cache = reward_scanner_cache()
        .lock()
        .map_err(|err| crate::OcrError::Processing(format!("could not lock OCR scanner: {err}")))?;

    let should_rebuild = cache
        .as_ref()
        .is_none_or(|cached| cached.theme != theme || cached.options != options);

    if should_rebuild {
        log::debug!("building cached reward screen OCR scanner for theme={theme}");
        let recognizer = TesseractRecognizer::new(&options)?;
        *cache = Some(CachedRewardScanner {
            theme,
            options: options.clone(),
            scanner: RewardScreenScanner::new(recognizer, theme, options),
        });
    } else {
        log::debug!("reusing cached reward screen OCR scanner for theme={theme}");
    }

    cache
        .as_ref()
        .expect("cached reward scanner should be initialized")
        .scanner
        .scan(frame)
}

fn reward_scanner_cache() -> &'static Mutex<Option<CachedRewardScanner>> {
    static CACHE: OnceLock<Mutex<Option<CachedRewardScanner>>> = OnceLock::new();

    CACHE.get_or_init(|| Mutex::new(None))
}

struct CachedRewardScanner {
    theme: RewardUiTheme,
    options: OcrOptions,
    scanner: RewardScreenScanner<TesseractRecognizer>,
}
