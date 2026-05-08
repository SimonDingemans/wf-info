use std::path::Path;

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

    let recognizer = TesseractRecognizer::new(&options)?;
    let scanner = RewardScreenScanner::new(recognizer, theme, options);

    scanner.scan(frame)
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
