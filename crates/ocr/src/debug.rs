use crate::implementations::reward_screen::{
    RewardNameCandidate, RewardUiTheme, scan_reward_screen_frame,
};
use crate::{CapturedFrame, OcrError, OcrOptions, Result};

const REWARD_SCREEN_FIXTURE: &[u8] = include_bytes!("../assets/test/RewardScreen_1.png");

pub fn scan_reward_screen_fixture(options: OcrOptions) -> Result<Vec<RewardNameCandidate>> {
    log::debug!("loading bundled reward screen OCR fixture");
    let frame = reward_screen_fixture_frame()?;

    log::debug!("running reward screen OCR against bundled fixture");
    scan_reward_screen_frame(&frame, RewardUiTheme::Vitruvian, options)
}

pub fn reward_screen_fixture_frame() -> Result<CapturedFrame> {
    image::load_from_memory(REWARD_SCREEN_FIXTURE)
        .map(CapturedFrame::new)
        .map_err(|err| {
            OcrError::ImageProcessing(format!(
                "could not load bundled reward screen OCR fixture: {err}"
            ))
        })
}
