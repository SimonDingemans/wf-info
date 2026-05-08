use thiserror::Error;

pub type Result<T> = std::result::Result<T, OcrError>;

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("failed to initialize OCR engine: {0}")]
    Initialization(String),

    #[error("OCR processing failed: {0}")]
    Processing(String),

    #[error("image processing failed: {0}")]
    ImageProcessing(String),

    #[error("capture failed: {0}")]
    Capture(String),

    #[error("unsupported OCR frame: {0}")]
    UnsupportedFrame(String),
}
