use image::DynamicImage;

use super::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureRequest {
    pub monitor: String,
}

#[derive(Clone, Debug)]
pub struct CapturedFrame {
    image: DynamicImage,
}

impl CapturedFrame {
    pub fn new(image: DynamicImage) -> Self {
        Self { image }
    }

    pub fn image(&self) -> &DynamicImage {
        &self.image
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn right(&self) -> u32 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(&self) -> u32 {
        self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanRegion {
    pub id: String,
    pub bounds: Rect,
}

impl ScanRegion {
    pub fn new(id: impl Into<String>, bounds: Rect) -> Self {
        Self {
            id: id.into(),
            bounds,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OcrImage {
    region: ScanRegion,
    image: DynamicImage,
}

impl OcrImage {
    pub fn new(region: ScanRegion, image: DynamicImage) -> Self {
        Self { region, image }
    }

    pub fn region(&self) -> &ScanRegion {
        &self.region
    }

    pub fn image(&self) -> &DynamicImage {
        &self.image
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcrOptions {
    pub language: String,
    pub tesseract_data_path: Option<String>,
    pub confidence_threshold: f32,
}

impl Default for OcrOptions {
    fn default() -> Self {
        Self {
            language: "eng".to_owned(),
            tesseract_data_path: None,
            confidence_threshold: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextCandidate {
    pub text: String,
    pub confidence: Option<f32>,
    pub region: Option<ScanRegion>,
}

impl TextCandidate {
    pub fn new(
        text: impl Into<String>,
        confidence: Option<f32>,
        region: Option<ScanRegion>,
    ) -> Self {
        Self {
            text: text.into(),
            confidence,
            region,
        }
    }
}

pub trait CaptureProvider {
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame>;
}

pub trait RegionDetector {
    fn detect_regions(&self, frame: &CapturedFrame) -> Result<Vec<ScanRegion>>;
}

pub trait ImagePreprocessor {
    fn preprocess(&self, frame: &CapturedFrame, region: &ScanRegion) -> Result<OcrImage>;
}

pub trait TextRecognizer {
    fn recognize(&self, image: &OcrImage, options: &OcrOptions) -> Result<Vec<TextCandidate>>;
}

pub trait FeatureScanner {
    type Output;

    fn scan(&self, frame: &CapturedFrame) -> Result<Self::Output>;
}
