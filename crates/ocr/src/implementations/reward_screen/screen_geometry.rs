pub const BASE_WIDTH: f32 = 1920.0;
pub const BASE_HEIGHT: f32 = 1080.0;

pub const PIXEL_REWARD_LINE_HEIGHT: f32 = 48.0;

pub fn screen_scaling(width: u32, height: u32) -> f32 {
    if width as f32 * BASE_HEIGHT > height as f32 * BASE_WIDTH {
        height as f32 / BASE_HEIGHT
    } else {
        width as f32 / BASE_WIDTH
    }
}

#[cfg(test)]
mod tests {
    use super::screen_scaling;

    #[test]
    fn scaling_uses_the_limiting_sixteen_by_nine_dimension() {
        assert_eq!(screen_scaling(1920, 1080), 1.0);
        assert_eq!(screen_scaling(3840, 2160), 2.0);
        assert_eq!(screen_scaling(2560, 1080), 1.0);
        assert_eq!(screen_scaling(1920, 1200), 1.0);
    }
}
