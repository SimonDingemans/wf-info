pub mod error;
pub mod implementations;
pub mod pipeline;
pub mod tesseract;

pub use error::{OcrError, Result};
pub use pipeline::{
    CaptureProvider, CaptureRequest, CapturedFrame, FeatureScanner, ImagePreprocessor, OcrImage,
    OcrOptions, Rect, RegionDetector, ScanRegion, TextCandidate, TextRecognizer,
};
