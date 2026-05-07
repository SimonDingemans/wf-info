mod scanner;
mod screen_geometry;
pub mod theme;

pub use scanner::{
    RewardNameCandidate, RewardNamePreprocessor, RewardNameRegionDetector, RewardScreenScanner,
    normalize_reward_ocr_text,
};
pub use theme::RewardUiTheme;
