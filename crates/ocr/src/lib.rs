pub mod capture;
pub mod debug;
pub mod error;
pub mod implementations;
pub mod pipeline;
pub mod tesseract;

pub use capture::PortalScreenshotCaptureProvider;
pub use error::{OcrError, Result};
pub use pipeline::{
    CaptureProvider, CaptureRegion, CaptureRequest, CapturedFrame, FeatureScanner,
    ImagePreprocessor, OcrImage, OcrOptions, Rect, RegionDetector, ScanRegion, TextCandidate,
    TextRecognizer,
};
