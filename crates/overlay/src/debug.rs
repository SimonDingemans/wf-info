use iced::widget::image::Handle as ImageHandle;
use iced::widget::{button, column, container, horizontal_rule, image, row, text};
use iced::{
    Color, Element, Font, Length, Pixels, Renderer, Subscription, Task as Command, Theme, time,
};
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::{Application, to_layer_message};
use shared::{
    AppContext, monitor,
    rewards::{RewardHighlight, RewardOverlayEntry},
};

const DEFAULT_MONITOR_WIDTH: u32 = 1920;
const DEFAULT_MONITOR_HEIGHT: u32 = 1080;
const REWARD_CARD_WIDTH: u32 = 180;
const REWARD_CARD_HEIGHT: u32 = 154;
const REWARD_CARD_SPACING: u32 = 10;
const REWARD_OVERLAY_PADDING: u32 = 18;
const REWARD_OVERLAY_Y_RATIO: f32 = 0.62;
const MONITOR_INFO_WIDTH: u32 = 520;
const MONITOR_INFO_HEIGHT: u32 = 260;
const PLATINUM_ICON: &[u8] = include_bytes!("../assets/src/PlatinumLarge.png");
const DUCAT_ICON: &[u8] = include_bytes!("../assets/src/OrokinDucats.png");

pub enum DebugOverlay {
    MonitorInfo {
        output_name: Option<String>,
        lines: Vec<String>,
    },
    Test {
        output_name: Option<String>,
    },
    Rewards {
        output_name: Option<String>,
        output_size: Option<(u32, u32)>,
        rewards: Vec<RewardOverlayEntry>,
    },
}

impl DebugOverlay {
    fn output_name(&self) -> Option<&str> {
        match self {
            Self::MonitorInfo { output_name, .. }
            | Self::Test { output_name }
            | Self::Rewards { output_name, .. } => output_name.as_deref(),
        }
    }
}

pub fn run(context: &AppContext, overlay: DebugOverlay) -> iced_layershell::Result {
    let start_mode = overlay
        .output_name()
        .map(|output| StartMode::TargetScreen(output.to_owned()))
        .unwrap_or(StartMode::Active);
    let layer_settings = layer_settings_for_overlay(&overlay, start_mode);

    DebugOverlayApp::run(Settings {
        id: Some(format!("{}.debug-overlay", context.name())),
        flags: overlay,
        layer_settings,
        fonts: Vec::new(),
        default_font: Font::default(),
        default_text_size: Pixels(16.0),
        antialiasing: true,
        virtual_keyboard_support: None,
    })
}

fn layer_settings_for_overlay(overlay: &DebugOverlay, start_mode: StartMode) -> LayerShellSettings {
    let mut settings = LayerShellSettings {
        layer: Layer::Overlay,
        exclusive_zone: -1,
        keyboard_interactivity: KeyboardInteractivity::None,
        events_transparent: false,
        start_mode,
        ..Default::default()
    };

    match overlay {
        DebugOverlay::Rewards {
            output_name,
            output_size,
            rewards,
        } => {
            let placement = reward_overlay_placement(
                (*output_size).or_else(|| detect_output_size(output_name.as_deref())),
                rewards.len(),
            );
            settings.anchor = Anchor::Top | Anchor::Left;
            settings.margin = placement.margin;
            settings.size = Some(placement.size);
        }
        DebugOverlay::MonitorInfo { .. } => {
            settings.anchor = Anchor::Top | Anchor::Right;
            settings.margin = (20, 20, 0, 0);
            settings.size = Some((MONITOR_INFO_WIDTH, MONITOR_INFO_HEIGHT));
            settings.keyboard_interactivity = KeyboardInteractivity::OnDemand;
        }
        DebugOverlay::Test { .. } => {
            settings.anchor = Anchor::Top | Anchor::Right | Anchor::Bottom | Anchor::Left;
            settings.keyboard_interactivity = KeyboardInteractivity::OnDemand;
        }
    }

    settings
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RewardOverlayPlacement {
    margin: (i32, i32, i32, i32),
    size: (u32, u32),
}

fn reward_overlay_placement(
    output_size: Option<(u32, u32)>,
    reward_count: usize,
) -> RewardOverlayPlacement {
    let (monitor_width, monitor_height) =
        output_size.unwrap_or((DEFAULT_MONITOR_WIDTH, DEFAULT_MONITOR_HEIGHT));
    let size = reward_overlay_surface_size(reward_count, monitor_width);
    let top = (monitor_height as f32 * REWARD_OVERLAY_Y_RATIO).round() as i32;
    let left = ((monitor_width.saturating_sub(size.0)) / 2) as i32;

    RewardOverlayPlacement {
        margin: (top, 0, 0, left),
        size,
    }
}

fn detect_output_size(output_name: Option<&str>) -> Option<(u32, u32)> {
    let Some(output_name) = output_name else {
        return None;
    };

    match monitor::detect_monitor_info() {
        Ok(monitors) => {
            let size = monitors
                .iter()
                .find(|monitor| monitor.matches_target(output_name))
                .and_then(|monitor| monitor.size)
                .and_then(|(width, height)| {
                    (width > 0 && height > 0).then_some((width as u32, height as u32))
                });

            if size.is_none() {
                log::warn!("could not find monitor size for reward overlay target {output_name}");
            }

            size
        }
        Err(err) => {
            log::warn!("could not detect monitor size for reward overlay placement: {err}");
            None
        }
    }
}

fn reward_overlay_surface_size(reward_count: usize, monitor_width: u32) -> (u32, u32) {
    let reward_count = reward_count.clamp(1, 4) as u32;
    let card_width = reward_count * REWARD_CARD_WIDTH;
    let spacing = reward_count.saturating_sub(1) * REWARD_CARD_SPACING;
    let width = (card_width + spacing + 2 * REWARD_OVERLAY_PADDING).min(monitor_width);
    let height = REWARD_CARD_HEIGHT + 2 * REWARD_OVERLAY_PADDING;

    (width, height)
}

struct DebugOverlayApp {
    overlay: DebugOverlay,
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    Close,
    Tick,
}

impl DebugOverlayApp {
    fn new(overlay: DebugOverlay) -> (Self, Command<Message>) {
        (Self { overlay }, Command::none())
    }

    fn namespace(&self) -> String {
        "wf-info debug overlay".to_owned()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        log::debug!("iced layershell overlay update event: {message:?}");

        match message {
            Message::Close => {
                Self::exit();
            }
            Message::Tick => {
                Self::exit();
            }
            _ => Command::none(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        time::every(std::time::Duration::from_secs(12)).map(|_| Message::Tick)
    }

    fn view(&self) -> Element<'_, Message, Theme, Renderer> {
        match &self.overlay {
            DebugOverlay::MonitorInfo { lines, .. } => monitor_info_view(lines),
            DebugOverlay::Test { .. } => test_overlay_view(),
            DebugOverlay::Rewards { rewards, .. } => reward_overlay_view(rewards),
        }
    }

    fn style(&self, _theme: &Theme) -> iced_layershell::Appearance {
        iced_layershell::Appearance {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
        }
    }
}

impl DebugOverlayApp {
    fn exit() -> ! {
        std::process::exit(0);
    }
}

impl Application for DebugOverlayApp {
    type Executor = iced::executor::Default;
    type Flags = DebugOverlay;
    type Message = Message;
    type Theme = Theme;

    fn new(overlay: Self::Flags) -> (Self, Command<Self::Message>) {
        DebugOverlayApp::new(overlay)
    }

    fn namespace(&self) -> String {
        DebugOverlayApp::namespace(self)
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        DebugOverlayApp::update(self, message)
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        DebugOverlayApp::subscription(self)
    }

    fn view(&self) -> Element<'_, Self::Message, Self::Theme, Renderer> {
        DebugOverlayApp::view(self)
    }

    fn style(&self, _theme: &Self::Theme) -> iced_layershell::Appearance {
        DebugOverlayApp::style(self, _theme)
    }
}

fn monitor_info_view(lines: &[String]) -> Element<'_, Message, Theme, Renderer> {
    let details = lines.iter().fold(column![].spacing(4), |column, line| {
        column.push(text(line).size(18))
    });

    container(
        column![
            text("wf-info monitor debug").size(24),
            horizontal_rule(1),
            details,
            button("Quit Overlay")
                .padding([8, 12])
                .on_press(Message::Close),
        ]
        .spacing(8),
    )
    .padding(18)
    .width(Length::Shrink)
    .height(Length::Shrink)
    .style(|_theme| container::Style {
        background: Some(Color::from_rgba(0.04, 0.05, 0.07, 0.82).into()),
        border: iced::Border {
            color: Color::from_rgb(0.22, 0.78, 0.72),
            width: 2.0,
            radius: 4.0.into(),
        },
        text_color: Some(Color::WHITE),
        ..Default::default()
    })
    .into()
}

fn test_overlay_view() -> Element<'static, Message, Theme, Renderer> {
    container(
        container(
            column![
                text("wf-info test overlay").size(36),
                text("Transparent layer-shell surface on the selected screen").size(20),
                button("Quit Overlay")
                    .padding([8, 12])
                    .on_press(Message::Close),
            ]
            .spacing(8),
        )
        .padding(24)
        .style(|_theme| container::Style {
            background: Some(Color::from_rgba(0.10, 0.23, 0.31, 0.72).into()),
            border: iced::Border {
                color: Color::from_rgb(1.0, 0.82, 0.22),
                width: 3.0,
                radius: 4.0.into(),
            },
            text_color: Some(Color::WHITE),
            ..Default::default()
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.18).into()),
        border: iced::Border {
            color: Color::from_rgb(1.0, 0.82, 0.22),
            width: 4.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn reward_overlay_view(rewards: &[RewardOverlayEntry]) -> Element<'_, Message, Theme, Renderer> {
    let best_platinum = best_platinum_reward_index(rewards);
    let reward_cards =
        rewards
            .iter()
            .enumerate()
            .fold(row![].spacing(10), |row, (index, reward)| {
                row.push(reward_card(
                    reward,
                    reward_is_best_platinum(index, reward, best_platinum),
                ))
            });

    container(reward_cards)
        .padding(18)
        .width(Length::Shrink)
        .height(Length::Shrink)
        .style(|_theme| container::Style {
            background: Some(Color::from_rgba(0.03, 0.04, 0.05, 0.78).into()),
            border: iced::Border {
                color: Color::from_rgb(0.20, 0.23, 0.27),
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: Some(Color::WHITE),
            ..Default::default()
        })
        .into()
}

fn reward_card(
    reward: &RewardOverlayEntry,
    is_best_platinum: bool,
) -> Element<'_, Message, Theme, Renderer> {
    let details = column![
        text(&reward.name).size(16).width(Length::Fill),
        reward_value_with_icon(reward.platinum, ImageHandle::from_bytes(PLATINUM_ICON)),
        reward_value_with_icon(reward.ducats, ImageHandle::from_bytes(DUCAT_ICON)),
        reward_detail("Sold last 48 hours", reward.volume),
    ]
    .spacing(4);
    let details = if reward.vaulted {
        details.push(
            text("Vaulted")
                .size(14)
                .color(Color::from_rgb(0.85, 0.78, 0.56)),
        )
    } else {
        details
    };

    let border_color = if is_best_platinum {
        Color::from_rgb(1.0, 0.84, 0.0)
    } else {
        Color::from_rgb(0.30, 0.34, 0.38)
    };

    let border_width = if is_best_platinum { 3.0 } else { 1.0 };

    container(details)
        .padding(12)
        .width(Length::Fixed(180.0))
        .style(move |_theme| container::Style {
            background: Some(Color::from_rgba(0.08, 0.09, 0.11, 0.92).into()),
            border: iced::Border {
                color: border_color,
                width: border_width,
                radius: 4.0.into(),
            },
            text_color: Some(Color::WHITE),
            ..Default::default()
        })
        .into()
}

fn reward_detail(
    label: &'static str,
    value: Option<u32>,
) -> Element<'static, Message, Theme, Renderer> {
    text(format!(
        "{label}: {}",
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "Unknown".to_owned())
    ))
    .size(14)
    .into()
}

fn reward_value_with_icon(
    value: Option<u32>,
    icon: ImageHandle,
) -> Element<'static, Message, Theme, Renderer> {
    row![
        image(icon).width(18).height(18),
        text(
            value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "Unknown".to_owned())
        )
        .size(14)
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn reward_is_best_platinum(
    index: usize,
    reward: &RewardOverlayEntry,
    best_platinum: Option<usize>,
) -> bool {
    reward.highlight == RewardHighlight::BestPlatinum || best_platinum == Some(index)
}

fn best_platinum_reward_index(rewards: &[RewardOverlayEntry]) -> Option<usize> {
    rewards
        .iter()
        .enumerate()
        .filter_map(|(index, reward)| reward.platinum.map(|platinum| (index, platinum)))
        .fold(None, |best, candidate| match best {
            Some((_, best_platinum)) if best_platinum >= candidate.1 => best,
            _ => Some(candidate),
        })
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use shared::rewards::{RewardHighlight, RewardOverlayEntry};

    use super::{
        DEFAULT_MONITOR_HEIGHT, DEFAULT_MONITOR_WIDTH, REWARD_CARD_HEIGHT, REWARD_CARD_SPACING,
        REWARD_CARD_WIDTH, REWARD_OVERLAY_PADDING, best_platinum_reward_index,
        reward_is_best_platinum, reward_overlay_placement, reward_overlay_surface_size,
    };

    #[test]
    fn best_platinum_reward_index_uses_highest_available_platinum_value() {
        let rewards = vec![
            RewardOverlayEntry::name_only("Forma Blueprint").with_platinum(8),
            RewardOverlayEntry::name_only("Braton Prime Receiver").with_platinum(42),
            RewardOverlayEntry::name_only("Paris Prime String").with_platinum(15),
        ];

        assert_eq!(best_platinum_reward_index(&rewards), Some(1));
    }

    #[test]
    fn best_platinum_reward_index_ignores_missing_platinum_values() {
        let rewards = vec![
            RewardOverlayEntry::name_only("Forma Blueprint"),
            RewardOverlayEntry::name_only("Braton Prime Receiver").with_platinum(12),
        ];

        assert_eq!(best_platinum_reward_index(&rewards), Some(1));
    }

    #[test]
    fn reward_highlight_can_mark_best_platinum_without_price_data() {
        let mut reward = RewardOverlayEntry::name_only("Forma Blueprint");
        reward.highlight = RewardHighlight::BestPlatinum;

        assert!(reward_is_best_platinum(0, &reward, None));
    }

    #[test]
    fn reward_overlay_surface_size_tracks_card_count() {
        let expected_width =
            4 * REWARD_CARD_WIDTH + 3 * REWARD_CARD_SPACING + 2 * REWARD_OVERLAY_PADDING;
        let expected_height = REWARD_CARD_HEIGHT + 2 * REWARD_OVERLAY_PADDING;

        assert_eq!(
            reward_overlay_surface_size(4, DEFAULT_MONITOR_WIDTH),
            (expected_width, expected_height)
        );
    }

    #[test]
    fn reward_overlay_surface_size_clamps_to_monitor_width() {
        assert_eq!(reward_overlay_surface_size(4, 320).0, 320);
    }

    #[test]
    fn reward_overlay_placement_uses_default_monitor_when_output_size_is_unknown() {
        let placement = reward_overlay_placement(None, 4);
        let expected_size = reward_overlay_surface_size(4, DEFAULT_MONITOR_WIDTH);

        assert_eq!(placement.size, expected_size);
        assert_eq!(
            placement.margin,
            (
                (DEFAULT_MONITOR_HEIGHT as f32 * super::REWARD_OVERLAY_Y_RATIO).round() as i32,
                0,
                0,
                ((DEFAULT_MONITOR_WIDTH - expected_size.0) / 2) as i32
            )
        );
    }
}
