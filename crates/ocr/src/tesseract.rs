use std::env;
use std::path::Path;
use std::sync::Mutex;

use kreuzberg_tesseract::{TessPageSegMode, TesseractAPI as Tesseract};

use super::pipeline::{OcrImage, OcrOptions, TextCandidate, TextRecognizer};
use super::{OcrError, Result};

pub struct TesseractRecognizer {
    engine: Mutex<Option<Tesseract>>,
}

impl TesseractRecognizer {
    pub fn new(options: &OcrOptions) -> Result<Self> {
        log::debug!(
            "creating Tesseract OCR recognizer with language={} configured_datapath={:?}",
            options.language,
            options.tesseract_data_path
        );

        let engine = Tesseract::new().map_err(|err| {
            OcrError::Initialization(format!("could not create Tesseract: {err}"))
        })?;
        let data_path = options
            .tesseract_data_path
            .clone()
            .unwrap_or_else(default_tessdata_path);

        log::debug!(
            "initializing Tesseract OCR engine with datapath={} language={}",
            data_path,
            options.language
        );

        engine.init(&data_path, &options.language).map_err(|err| {
            OcrError::Initialization(format!(
                "could not initialize Tesseract with datapath {data_path}: {err}"
            ))
        })?;
        engine
            .set_page_seg_mode(TessPageSegMode::PSM_SINGLE_BLOCK)
            .map_err(|err| {
                OcrError::Initialization(format!(
                    "could not configure Tesseract page segmentation mode: {err}"
                ))
            })?;

        log::debug!("Tesseract OCR recognizer initialized");

        Ok(Self {
            engine: Mutex::new(Some(engine)),
        })
    }
}

impl TextRecognizer for TesseractRecognizer {
    fn recognize(&self, image: &OcrImage, _options: &OcrOptions) -> Result<Vec<TextCandidate>> {
        log::debug!(
            "running Tesseract OCR for region={} bounds={:?} image={}x{}",
            image.region().id,
            image.region().bounds,
            image.image().width(),
            image.image().height()
        );

        let engine = self
            .engine
            .lock()
            .map_err(|err| OcrError::Processing(format!("could not lock Tesseract: {err}")))?;
        let engine = engine
            .as_ref()
            .ok_or_else(|| OcrError::Initialization("Tesseract instance is missing".to_owned()))?;
        let rgb_image = image.image().to_rgb8();
        let buffer = rgb_image.as_flat_samples();

        engine
            .set_image(
                buffer.samples,
                rgb_image.width() as i32,
                rgb_image.height() as i32,
                3,
                3 * rgb_image.width() as i32,
            )
            .map_err(|err| OcrError::Processing(format!("could not set OCR image: {err}")))?;

        let text = engine
            .get_utf8_text()
            .map_err(|err| OcrError::Processing(format!("could not read OCR text: {err}")))?;
        let _ = engine.clear();

        log::debug!(
            "Tesseract OCR completed for region={} text_len={} preview={:?}",
            image.region().id,
            text.len(),
            text_preview(&text)
        );

        Ok(vec![TextCandidate::new(
            text,
            None,
            Some(image.region().clone()),
        )])
    }
}

fn default_tessdata_path() -> String {
    if let Ok(prefix) = env::var("TESSDATA_PREFIX") {
        log::debug!("using TESSDATA_PREFIX for Tesseract datapath: {prefix}");
        return prefix;
    }

    let local_path = Path::new("tessdata");
    if local_path.exists() && local_path.is_dir() && local_path.join("eng.traineddata").exists() {
        log::debug!("using local Tesseract datapath: {}", local_path.display());
        return local_path.to_string_lossy().to_string();
    }

    if let Ok(exe_path) = env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let exe_tessdata = exe_dir.join("tessdata");
        if exe_tessdata.exists()
            && exe_tessdata.is_dir()
            && exe_tessdata.join("eng.traineddata").exists()
        {
            log::debug!(
                "using executable-adjacent Tesseract datapath: {}",
                exe_tessdata.display()
            );
            return exe_tessdata.to_string_lossy().to_string();
        }
    }

    log::debug!("using fallback Tesseract datapath: /usr/share/tessdata");
    "/usr/share/tessdata".to_owned()
}

fn text_preview(text: &str) -> String {
    const MAX_CHARS: usize = 80;

    let mut preview = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if preview.chars().count() > MAX_CHARS {
        preview = preview.chars().take(MAX_CHARS).collect::<String>();
        preview.push_str("...");
    }

    preview
}
