use std::f32::consts::PI;

use image::{DynamicImage, GenericImageView, Pixel, Rgb};

use crate::pipeline::{
    CapturedFrame, FeatureScanner, ImagePreprocessor, OcrImage, OcrOptions, Rect, RegionDetector,
    ScanRegion, TextRecognizer,
};
use crate::{OcrError, Result};

use super::screen_geometry::{PIXEL_REWARD_LINE_HEIGHT, screen_scaling};
use super::theme::RewardUiTheme;

const PIXEL_REWARD_WIDTH: f32 = 968.0;
const PIXEL_REWARD_HEIGHT: f32 = 235.0;
const PIXEL_REWARD_YDISPLAY: f32 = 316.0;

const TEXT_SEGMENTS: [f32; 4] = [2.0, 4.0, 16.0, 21.0];

const RATIO_TOP: f32 = 0.06;
const RATIO_MID_LOW: f32 = 0.24;
const RATIO_MID_HIGH: f32 = 0.26;
const RATIO_BOT: f32 = 0.007;

#[derive(Clone, Debug)]
pub struct RewardNameRegionDetector {
    theme: RewardUiTheme,
}

impl RewardNameRegionDetector {
    pub fn new(theme: RewardUiTheme) -> Self {
        Self { theme }
    }
}

impl RegionDetector for RewardNameRegionDetector {
    fn detect_regions(&self, frame: &CapturedFrame) -> Result<Vec<ScanRegion>> {
        let frame_size = FrameSize::new(frame.width(), frame.height())?;
        let scaling = screen_scaling(frame.width(), frame.height());
        let prefilter_bounds = calculate_prefilter_bounds(frame_size, scaling)?;
        let prefilter = crop(frame.image(), prefilter_bounds)?;
        let rows = calculate_row_histogram(&prefilter, self.theme);
        let reward_text_scale =
            find_best_reward_text_scale(prefilter.height(), prefilter.width(), &rows, scaling);
        let reward_line_bounds =
            calculate_reward_line_bounds(frame_size, prefilter_bounds, scaling, reward_text_scale)?;
        let layout = detect_reward_layout(frame.image(), reward_line_bounds, self.theme)?;

        Ok((0..layout.player_count)
            .map(|index| {
                let bounds = Rect {
                    x: reward_line_bounds.x + layout.left_offset + index * layout.slot_width,
                    y: reward_line_bounds.y,
                    width: layout.slot_width,
                    height: reward_line_bounds.height,
                };

                ScanRegion::new(format!("reward-name-{index}"), bounds)
            })
            .collect())
    }
}

#[derive(Clone, Debug)]
pub struct RewardNamePreprocessor {
    theme: RewardUiTheme,
}

impl RewardNamePreprocessor {
    pub fn new(theme: RewardUiTheme) -> Self {
        Self { theme }
    }
}

impl ImagePreprocessor for RewardNamePreprocessor {
    fn preprocess(&self, frame: &CapturedFrame, region: &ScanRegion) -> Result<OcrImage> {
        let mut filtered = crop(frame.image(), region.bounds)?.into_rgb8();

        for pixel in filtered.pixels_mut() {
            if self.theme.threshold_filter(*pixel) {
                *pixel = Rgb([0, 0, 0]);
            } else {
                *pixel = Rgb([255, 255, 255]);
            }
        }

        Ok(OcrImage::new(
            region.clone(),
            DynamicImage::ImageRgb8(filtered),
        ))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RewardNameCandidate {
    pub raw_text: String,
    pub normalized_text: String,
    pub confidence: Option<f32>,
    pub region: ScanRegion,
}

pub struct RewardScreenScanner<R> {
    detector: RewardNameRegionDetector,
    preprocessor: RewardNamePreprocessor,
    recognizer: R,
    options: OcrOptions,
}

impl<R> RewardScreenScanner<R> {
    pub fn new(recognizer: R, theme: RewardUiTheme, options: OcrOptions) -> Self {
        Self {
            detector: RewardNameRegionDetector::new(theme),
            preprocessor: RewardNamePreprocessor::new(theme),
            recognizer,
            options,
        }
    }
}

impl<R> FeatureScanner for RewardScreenScanner<R>
where
    R: TextRecognizer,
{
    type Output = Vec<RewardNameCandidate>;

    fn scan(&self, frame: &CapturedFrame) -> Result<Self::Output> {
        let regions = self.detector.detect_regions(frame)?;
        let mut reward_names = Vec::new();

        for region in regions {
            let image = self.preprocessor.preprocess(frame, &region)?;
            let candidates = self.recognizer.recognize(&image, &self.options)?;
            reward_names.extend(candidates.into_iter().map(|candidate| {
                let normalized_text = normalize_reward_ocr_text(&candidate.text);

                RewardNameCandidate {
                    raw_text: candidate.text,
                    normalized_text,
                    confidence: candidate.confidence,
                    region: candidate.region.unwrap_or_else(|| region.clone()),
                }
            }));
        }

        Ok(reward_names)
    }
}

pub fn normalize_reward_ocr_text(text: &str) -> String {
    text.replace(|character: char| !character.is_ascii_alphabetic(), "")
}

#[derive(Clone, Copy, Debug)]
struct FrameSize {
    width: u32,
    height: u32,
}

impl FrameSize {
    fn new(width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(OcrError::UnsupportedFrame(
                "reward OCR needs a non-empty captured frame".to_owned(),
            ));
        }

        Ok(Self { width, height })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RewardLayout {
    left_offset: u32,
    slot_width: u32,
    player_count: u32,
}

fn calculate_prefilter_bounds(size: FrameSize, scaling: f32) -> Result<Rect> {
    let width = size.width as f32;
    let height = size.height as f32;
    let prefilter_width = PIXEL_REWARD_WIDTH * scaling;
    let prefilter_left = width / 2.0 - prefilter_width / 2.0;
    let prefilter_top = height / 2.0
        - ((PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT + PIXEL_REWARD_LINE_HEIGHT) * scaling);
    let prefilter_bottom =
        height / 2.0 - ((PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT) * scaling * 0.5);

    rect_from_f32(
        prefilter_left,
        prefilter_top,
        prefilter_width,
        prefilter_bottom - prefilter_top,
    )
}

fn calculate_reward_line_bounds(
    size: FrameSize,
    prefilter_bounds: Rect,
    screen_scaling: f32,
    reward_text_scale: f32,
) -> Result<Rect> {
    let high_scaling = if reward_text_scale < 1.0 {
        reward_text_scale + 0.01
    } else {
        reward_text_scale
    };
    let low_scaling = if reward_text_scale > 0.5 {
        reward_text_scale + 0.01
    } else {
        reward_text_scale
    };

    let crop_width = PIXEL_REWARD_WIDTH * screen_scaling * high_scaling;
    let crop_left =
        prefilter_bounds.x as f32 + prefilter_bounds.width as f32 / 2.0 - crop_width / 2.0;
    let crop_top = size.height as f32 / 2.0
        - (PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT + PIXEL_REWARD_LINE_HEIGHT)
            * screen_scaling
            * high_scaling;
    let crop_bottom = size.height as f32 / 2.0
        - (PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT) * screen_scaling * low_scaling;

    rect_from_f32(crop_left, crop_top, crop_width, crop_bottom - crop_top)
}

fn detect_reward_layout(
    image: &DynamicImage,
    reward_line_bounds: Rect,
    theme: RewardUiTheme,
) -> Result<RewardLayout> {
    let reward_line = crop(image, reward_line_bounds)?.into_rgb8();
    let mut total_even = 0.0;
    let mut total_odd = 0.0;
    let width = reward_line.width();
    let height = reward_line.height();

    for x in 0..width {
        let mut count = 0;
        for y in 0..height {
            if theme.threshold_filter(*reward_line.get_pixel(x, y)) {
                count += 1;
            }
        }

        let count = count.min(height / 3);
        let cosine = (8.0 * x as f32 * PI / width as f32).cos();
        let weight = cosine.powi(3) * count as f32;

        if cosine < 0.0 {
            total_even -= weight;
        } else if cosine > 0.0 {
            total_odd += weight;
        }
    }

    if total_even == 0.0 && total_odd == 0.0 {
        return Ok(RewardLayout {
            left_offset: 0,
            slot_width: width / 4,
            player_count: 0,
        });
    }

    let slot_width = width / 4;
    let (left_offset, player_count) = if total_odd > total_even {
        (slot_width / 2, 3)
    } else {
        (0, 4)
    };

    Ok(RewardLayout {
        left_offset,
        slot_width,
        player_count,
    })
}

fn calculate_row_histogram(image: &DynamicImage, theme: RewardUiTheme) -> Vec<usize> {
    (0..image.height())
        .map(|y| {
            (0..image.width())
                .filter(|&x| theme.threshold_filter(image.get_pixel(x, y).to_rgb()))
                .count()
        })
        .collect()
}

fn find_best_reward_text_scale(
    image_height: u32,
    image_width: u32,
    rows: &[usize],
    screen_scaling: f32,
) -> f32 {
    let line_height = (PIXEL_REWARD_LINE_HEIGHT / 2.0 * screen_scaling) as usize;
    let top_line_100 = image_height as usize - line_height;
    let top_line_50 = line_height / 2;

    let mut best_scale = 50;
    let mut lowest_weight = f32::MAX;

    for index in 0..50 {
        let scale = 50 + index;
        let scale_f = scale as f32 / 100.0;
        let scale_width = image_width as f32 * scale_f;
        let y_from_top = image_height as usize
            - (index as f32 * (top_line_100 - top_line_50) as f32 / 50.0 + top_line_50 as f32)
                as usize;

        let text_top = (screen_scaling * TEXT_SEGMENTS[0] * scale_f) as usize;
        let text_top_bottom = (screen_scaling * TEXT_SEGMENTS[1] * scale_f) as usize;
        let text_body_bottom = (screen_scaling * TEXT_SEGMENTS[2] * scale_f) as usize;
        let text_tail_bottom = (screen_scaling * TEXT_SEGMENTS[3] * scale_f) as usize;

        let top_weight = average_weight(text_top..=text_top_bottom, |offset| {
            (scale_width * RATIO_TOP - rows[y_from_top + offset] as f32).abs()
        });
        let mid_weight = average_weight(text_top_bottom + 1..text_body_bottom, |offset| {
            let row_fill = rows[y_from_top + offset] as f32;
            if row_fill < scale_width / 15.0 {
                (scale_width * RATIO_MID_HIGH - row_fill) * 5.0
            } else {
                (scale_width * RATIO_MID_LOW - row_fill).abs()
            }
        });
        let bottom_weight = average_weight(text_body_bottom..text_tail_bottom, |offset| {
            10.0 * (scale_width * RATIO_BOT - rows[y_from_top + offset] as f32).abs()
        });

        let total_weight = top_weight + mid_weight + bottom_weight;
        if total_weight < lowest_weight {
            lowest_weight = total_weight;
            best_scale = scale;
        }
    }

    best_scale as f32 / 100.0
}

fn average_weight(range: impl Iterator<Item = usize>, weight: impl Fn(usize) -> f32) -> f32 {
    let mut total = 0.0;
    let mut count = 0;

    for offset in range {
        total += weight(offset);
        count += 1;
    }

    if count == 0 {
        return 0.0;
    }

    total / count as f32
}

fn crop(image: &DynamicImage, bounds: Rect) -> Result<DynamicImage> {
    if bounds.right() > image.width() || bounds.bottom() > image.height() {
        return Err(OcrError::UnsupportedFrame(format!(
            "scan region {:?} is outside captured frame {}x{}",
            bounds,
            image.width(),
            image.height()
        )));
    }

    Ok(image.crop_imm(bounds.x, bounds.y, bounds.width, bounds.height))
}

fn rect_from_f32(x: f32, y: f32, width: f32, height: f32) -> Result<Rect> {
    if x < 0.0 || y < 0.0 || width <= 0.0 || height <= 0.0 {
        return Err(OcrError::UnsupportedFrame(format!(
            "invalid reward OCR bounds x={x}, y={y}, width={width}, height={height}"
        )));
    }

    Ok(Rect {
        x: x as u32,
        y: y as u32,
        width: width as u32,
        height: height as u32,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        CapturedFrame, ImagePreprocessor, Rect, RewardLayout, RewardNamePreprocessor,
        RewardNameRegionDetector, RewardScreenScanner, RewardUiTheme, ScanRegion,
        detect_reward_layout, normalize_reward_ocr_text,
    };
    use crate::tesseract::TesseractRecognizer;
    use crate::{FeatureScanner, OcrOptions, RegionDetector};
    use image::{DynamicImage, Rgb, RgbImage};

    #[test]
    fn normalizes_reward_ocr_text_to_matchable_ascii_name() {
        assert_eq!(
            normalize_reward_ocr_text("  Braton Prime\nReceiver (Rare)! "),
            "BratonPrimeReceiverRare"
        );
    }

    #[test]
    fn preprocessor_turns_theme_text_black_and_background_white() {
        let mut image = RgbImage::from_pixel(4, 2, Rgb([12, 12, 12]));
        image.put_pixel(1, 0, Rgb([36, 184, 242]));
        let frame = CapturedFrame::new(DynamicImage::ImageRgb8(image));
        let region = ScanRegion::new(
            "reward-name-0",
            Rect {
                x: 0,
                y: 0,
                width: 4,
                height: 2,
            },
        );
        let preprocessor = RewardNamePreprocessor::new(RewardUiTheme::Lotus);
        let ocr_image = preprocessor
            .preprocess(&frame, &region)
            .expect("region should preprocess");
        let filtered = ocr_image.image().to_rgb8();

        assert_eq!(*filtered.get_pixel(1, 0), Rgb([0, 0, 0]));
        assert_eq!(*filtered.get_pixel(0, 0), Rgb([255, 255, 255]));
    }

    #[test]
    fn layout_detects_three_player_reward_rows_from_centered_columns() {
        let mut image = RgbImage::from_pixel(80, 12, Rgb([0, 0, 0]));
        for x in [14, 15, 16, 34, 35, 36, 54, 55, 56] {
            for y in 2..8 {
                image.put_pixel(x, y, Rgb([36, 184, 242]));
            }
        }

        let layout = detect_reward_layout(
            &DynamicImage::ImageRgb8(image),
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 12,
            },
            RewardUiTheme::Lotus,
        )
        .expect("layout should be detected");

        assert_eq!(
            layout,
            RewardLayout {
                left_offset: 10,
                slot_width: 20,
                player_count: 3
            }
        );
    }

    #[test]
    fn detects_and_preprocesses_reward_names_from_real_reward_screen() {
        let frame = load_reward_screen_fixture();
        let detector = RewardNameRegionDetector::new(RewardUiTheme::Vitruvian);
        let regions = detector
            .detect_regions(&frame)
            .expect("reward regions should be detected from fixture");

        assert_eq!(regions.len(), 4);
        assert!(
            regions
                .windows(2)
                .all(|pair| pair[0].bounds.x < pair[1].bounds.x)
        );

        let preprocessor = RewardNamePreprocessor::new(RewardUiTheme::Vitruvian);
        for region in regions {
            let ocr_image = preprocessor
                .preprocess(&frame, &region)
                .expect("reward name region should preprocess");
            let image = ocr_image.image().to_rgb8();
            let black_pixels = image
                .pixels()
                .filter(|pixel| **pixel == Rgb([0, 0, 0]))
                .count();
            let white_pixels = image
                .pixels()
                .filter(|pixel| **pixel == Rgb([255, 255, 255]))
                .count();

            assert_eq!(image.width(), region.bounds.width);
            assert_eq!(image.height(), region.bounds.height);
            assert!(
                black_pixels > 0,
                "{} should contain OCR text pixels",
                region.id
            );
            assert_eq!(
                black_pixels + white_pixels,
                image.pixels().count(),
                "{} should be a binary OCR crop",
                region.id
            );
        }
    }

    #[test]
    fn reads_expected_reward_names_from_real_reward_screen() {
        let options = OcrOptions::default();
        let recognizer =
            TesseractRecognizer::new(&options).expect("Tesseract should initialize for OCR test");
        let scanner = RewardScreenScanner::new(recognizer, RewardUiTheme::Vitruvian, options);
        let rewards = scanner
            .scan(&load_reward_screen_fixture())
            .expect("fixture reward names should OCR");
        let normalized_rewards = rewards
            .iter()
            .map(|reward| reward.normalized_text.as_str())
            .collect::<Vec<_>>();
        let expected_rewards = [
            "SarofangPrimeHandle",
            "FormaBlueprint",
            "GrendelPrimeBlueprint",
            "NautilusPrimeSystems",
        ]
        .map(normalize_reward_ocr_text);
        let expected_rewards = expected_rewards
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();

        assert_eq!(normalized_rewards, expected_rewards);
    }

    fn load_reward_screen_fixture() -> CapturedFrame {
        let path = reward_screen_fixture_path();
        let image = image::open(&path).unwrap_or_else(|err| {
            panic!(
                "failed to open reward screen fixture at {}: {err}",
                path.display()
            )
        });

        CapturedFrame::new(image)
    }

    fn reward_screen_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/test/RewardScreen_1.png")
    }
}
