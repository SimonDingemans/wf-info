use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::implementations::reward_screen::{RewardNameCandidate, RewardScreenScanner};
use crate::tesseract::TesseractRecognizer;
use crate::{CapturedFrame, FeatureScanner, ImagePreprocessor, OcrOptions, RegionDetector, Result};
use crate::{OcrError, TextRecognizer};

use super::{RewardNamePreprocessor, RewardNameRegionDetector, RewardUiTheme};

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

    fs::create_dir_all(debug_dir).map_err(|err| {
        OcrError::ImageProcessing(format!(
            "could not create reward OCR debug directory {}: {err}",
            debug_dir.display()
        ))
    })?;

    save_debug_image(frame.image(), debug_dir.join("latest-ocr-frame.png"))?;

    let detector = RewardNameRegionDetector::new(theme);
    let preprocessor = RewardNamePreprocessor::new(theme);
    let recognizer = TesseractRecognizer::new(&options)?;
    let regions = detector.detect_regions(frame)?;
    if regions.is_empty() {
        log::debug!(
            "configured reward UI theme {} detected no reward name regions; probing other themes for diagnostics",
            theme
        );
        log_theme_region_probe(frame, theme);
    }
    let mut reward_names = Vec::new();

    log::debug!(
        "reward screen OCR debug scan will process {} region(s)",
        regions.len()
    );

    for region in regions {
        let image = preprocessor.preprocess(frame, &region)?;
        let safe_region_id = safe_filename_part(&region.id);
        save_debug_image(
            image.image(),
            debug_dir.join(format!("latest-{safe_region_id}-tesseract-input.png")),
        )?;
        save_debug_image(
            image.image(),
            debug_dir.join(format!("{safe_region_id}-tesseract-input.png")),
        )?;

        let candidates = recognizer.recognize(&image, &options)?;
        reward_names.extend(candidates.into_iter().map(|candidate| {
            let normalized_text = super::normalize_reward_ocr_text(&candidate.text);

            log::debug!(
                "reward OCR candidate for {}: raw_len={} normalized={:?} confidence={:?}",
                region.id,
                candidate.text.len(),
                normalized_text,
                candidate.confidence
            );

            RewardNameCandidate {
                raw_text: candidate.text,
                normalized_text,
                confidence: candidate.confidence,
                region: candidate.region.unwrap_or_else(|| region.clone()),
            }
        }));
    }

    log::debug!(
        "reward screen OCR debug scan completed with {} reward name candidate(s)",
        reward_names.len()
    );

    Ok(reward_names)
}

fn log_theme_region_probe(frame: &CapturedFrame, configured_theme: RewardUiTheme) {
    for theme in RewardUiTheme::ALL {
        if theme == configured_theme {
            continue;
        }

        let detector = RewardNameRegionDetector::new(theme);
        match detector.detect_regions(frame) {
            Ok(regions) if regions.is_empty() => {}
            Ok(regions) => {
                log::debug!(
                    "reward UI theme probe: theme={} detected {} reward name region(s)",
                    theme,
                    regions.len()
                );
            }
            Err(err) => {
                log::debug!("reward UI theme probe failed for theme={theme}: {err}");
            }
        }
    }
}

fn save_debug_image(image: &image::DynamicImage, path: PathBuf) -> Result<()> {
    image.save(&path).map_err(|err| {
        OcrError::ImageProcessing(format!(
            "could not save reward OCR debug image {}: {err}",
            path.display()
        ))
    })?;
    log::debug!("saved reward OCR debug image to {}", path.display());
    Ok(())
}

fn safe_filename_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect()
}
