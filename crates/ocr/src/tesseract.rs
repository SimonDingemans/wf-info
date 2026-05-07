use std::env;
use std::path::Path;
use std::sync::Mutex;

use kreuzberg_tesseract::TesseractAPI as Tesseract;

use super::pipeline::{OcrImage, OcrOptions, TextCandidate, TextRecognizer};
use super::{OcrError, Result};

pub struct TesseractRecognizer {
    engine: Mutex<Option<Tesseract>>,
}

impl TesseractRecognizer {
    pub fn new(options: &OcrOptions) -> Result<Self> {
        let engine = Tesseract::new().map_err(|err| {
            OcrError::Initialization(format!("could not create Tesseract: {err}"))
        })?;
        let data_path = options
            .tesseract_data_path
            .clone()
            .unwrap_or_else(default_tessdata_path);

        engine.init(&data_path, &options.language).map_err(|err| {
            OcrError::Initialization(format!(
                "could not initialize Tesseract with datapath {data_path}: {err}"
            ))
        })?;

        Ok(Self {
            engine: Mutex::new(Some(engine)),
        })
    }
}

impl TextRecognizer for TesseractRecognizer {
    fn recognize(&self, image: &OcrImage, _options: &OcrOptions) -> Result<Vec<TextCandidate>> {
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

        Ok(vec![TextCandidate::new(
            text,
            None,
            Some(image.region().clone()),
        )])
    }
}

fn default_tessdata_path() -> String {
    if let Ok(prefix) = env::var("TESSDATA_PREFIX") {
        return prefix;
    }

    let local_path = Path::new("tessdata");
    if local_path.exists() && local_path.is_dir() && local_path.join("eng.traineddata").exists() {
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
            return exe_tessdata.to_string_lossy().to_string();
        }
    }

    "/usr/share/tessdata".to_owned()
}
