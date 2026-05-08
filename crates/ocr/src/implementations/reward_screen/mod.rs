mod scanner;
mod screen_geometry;
mod service;
pub mod theme;

pub use scanner::{
    RewardNameCandidate, RewardNamePreprocessor, RewardNameRegionDetector, RewardScreenScanner,
    normalize_reward_ocr_text,
};
pub use service::{scan_reward_screen_frame, scan_reward_screen_frame_with_debug_images};
pub use theme::RewardUiTheme;
