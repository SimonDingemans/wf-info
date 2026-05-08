use std::fmt;

use image::Rgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewardUiTheme {
    Vitruvian,
    Stalker,
    Baruuk,
    Corpus,
    Fortuna,
    Grineer,
    Lotus,
    Nidus,
    Orokin,
    Tenno,
    HighContrast,
    Legacy,
    Equinox,
    DarkLotus,
    Zephyr,
}

impl RewardUiTheme {
    pub const ALL: [Self; 15] = [
        Self::Vitruvian,
        Self::Stalker,
        Self::Baruuk,
        Self::Corpus,
        Self::Fortuna,
        Self::Grineer,
        Self::Lotus,
        Self::Nidus,
        Self::Orokin,
        Self::Tenno,
        Self::HighContrast,
        Self::Legacy,
        Self::Equinox,
        Self::DarkLotus,
        Self::Zephyr,
    ];

    pub fn from_config_key(value: &str) -> Option<Self> {
        let normalized = value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>();

        match normalized.as_str() {
            "vitruvian" => Some(Self::Vitruvian),
            "stalker" => Some(Self::Stalker),
            "baruuk" => Some(Self::Baruuk),
            "corpus" => Some(Self::Corpus),
            "fortuna" => Some(Self::Fortuna),
            "grineer" => Some(Self::Grineer),
            "lotus" => Some(Self::Lotus),
            "nidus" => Some(Self::Nidus),
            "orokin" => Some(Self::Orokin),
            "tenno" => Some(Self::Tenno),
            "highcontrast" => Some(Self::HighContrast),
            "legacy" => Some(Self::Legacy),
            "equinox" => Some(Self::Equinox),
            "darklotus" => Some(Self::DarkLotus),
            "zephyr" => Some(Self::Zephyr),
            _ => None,
        }
    }

    pub fn threshold_filter(&self, color: Rgb<u8>) -> bool {
        let hsl = HslColor::from_rgb(color);

        match self {
            Self::Equinox => hsl.saturation <= 0.2 && hsl.lightness >= 0.55,
            Self::Stalker => {
                (0.61..1.00).contains(&hsl.saturation)
                    && (0.25..0.65).contains(&hsl.lightness)
                    && (-10.0..5.0).contains(&hsl.hue)
            }
            Self::HighContrast => {
                hsl.saturation >= 0.60
                    && (0.23..0.45).contains(&hsl.lightness)
                    && (-160.0..-145.0).contains(&hsl.hue)
            }
            _ => self.theme_colors().is_some_and(|(primary, secondary)| {
                rgb_difference(primary, color) < 0.2 || rgb_difference(secondary, color) < 0.2
            }),
        }
    }

    fn theme_colors(&self) -> Option<(Rgb<u8>, Rgb<u8>)> {
        let colors = match self {
            Self::Vitruvian => ((190, 169, 102), (245, 227, 173)),
            Self::Baruuk => ((238, 193, 105), (236, 211, 162)),
            Self::Corpus => ((35, 201, 245), (111, 229, 253)),
            Self::Fortuna => ((57, 105, 192), (255, 115, 230)),
            Self::Grineer => ((255, 189, 102), (255, 224, 153)),
            Self::Lotus => ((36, 184, 242), (255, 241, 191)),
            Self::Nidus => ((140, 38, 92), (245, 73, 93)),
            Self::Orokin => ((20, 41, 29), (178, 125, 5)),
            Self::Tenno => ((9, 78, 106), (6, 106, 74)),
            Self::Legacy => ((255, 255, 255), (232, 213, 93)),
            Self::DarkLotus => ((140, 119, 147), (189, 169, 237)),
            Self::Zephyr => ((253, 132, 2), (255, 53, 0)),
            Self::Stalker | Self::HighContrast | Self::Equinox => return None,
        };

        Some((rgb(colors.0), rgb(colors.1)))
    }
}

impl fmt::Display for RewardUiTheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Vitruvian => "Vitruvian",
            Self::Stalker => "Stalker",
            Self::Baruuk => "Baruuk",
            Self::Corpus => "Corpus",
            Self::Fortuna => "Fortuna",
            Self::Grineer => "Grineer",
            Self::Lotus => "Lotus",
            Self::Nidus => "Nidus",
            Self::Orokin => "Orokin",
            Self::Tenno => "Tenno",
            Self::HighContrast => "High Contrast",
            Self::Legacy => "Legacy",
            Self::Equinox => "Equinox",
            Self::DarkLotus => "Dark Lotus",
            Self::Zephyr => "Zephyr",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HslColor {
    hue: f32,
    saturation: f32,
    lightness: f32,
}

impl HslColor {
    fn from_rgb(color: Rgb<u8>) -> Self {
        let red = f32::from(color.0[0]) / 255.0;
        let green = f32::from(color.0[1]) / 255.0;
        let blue = f32::from(color.0[2]) / 255.0;

        let max = red.max(green).max(blue);
        let min = red.min(green).min(blue);
        let lightness = (max + min) / 2.0;

        if max == min {
            return Self {
                hue: 0.0,
                saturation: 0.0,
                lightness,
            };
        }

        let delta = max - min;
        let saturation = if lightness > 0.5 {
            delta / (2.0 - max - min)
        } else {
            delta / (max + min)
        };

        let mut hue = if max == red {
            (green - blue) / delta
        } else if max == green {
            (blue - red) / delta + 2.0
        } else {
            (red - green) / delta + 4.0
        } * 60.0;

        if hue > 180.0 {
            hue -= 360.0;
        }

        Self {
            hue,
            saturation,
            lightness,
        }
    }
}

fn rgb(value: (u8, u8, u8)) -> Rgb<u8> {
    Rgb([value.0, value.1, value.2])
}

fn rgb_difference(left: Rgb<u8>, right: Rgb<u8>) -> f32 {
    left.0
        .iter()
        .zip(right.0.iter())
        .map(|(left, right)| (i16::from(*left) - i16::from(*right)).abs() as f32)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::RewardUiTheme;
    use image::Rgb;

    #[test]
    fn parses_config_theme_names_flexibly() {
        assert_eq!(
            RewardUiTheme::from_config_key("dark_lotus"),
            Some(RewardUiTheme::DarkLotus)
        );
        assert_eq!(
            RewardUiTheme::from_config_key("High Contrast"),
            Some(RewardUiTheme::HighContrast)
        );
        assert_eq!(RewardUiTheme::from_config_key("unknown"), None);
    }

    #[test]
    fn all_themes_contains_each_configurable_theme() {
        assert_eq!(RewardUiTheme::ALL.len(), 15);
        assert!(RewardUiTheme::ALL.contains(&RewardUiTheme::Lotus));
        assert!(RewardUiTheme::ALL.contains(&RewardUiTheme::Vitruvian));
        assert!(RewardUiTheme::ALL.contains(&RewardUiTheme::Zephyr));
    }

    #[test]
    fn lotus_threshold_matches_known_reward_text_colors() {
        let lotus = RewardUiTheme::Lotus;

        assert!(lotus.threshold_filter(Rgb([36, 184, 242])));
        assert!(lotus.threshold_filter(Rgb([255, 241, 191])));
        assert!(!lotus.threshold_filter(Rgb([12, 12, 12])));
    }
}
