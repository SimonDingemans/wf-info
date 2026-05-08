use iced::widget::{button, column, container, horizontal_rule, row, text};
use iced::{
    Color, Element, Font, Length, Pixels, Renderer, Subscription, Task as Command, Theme, time,
};
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::{Application, to_layer_message};
use shared::{AppContext, rewards::RewardOverlayEntry};

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

    DebugOverlayApp::run(Settings {
        id: Some(format!("{}.debug-overlay", context.name())),
        flags: overlay,
        layer_settings: LayerShellSettings {
            anchor: Anchor::Top | Anchor::Right | Anchor::Bottom | Anchor::Left,
            layer: Layer::Overlay,
            exclusive_zone: -1,
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
            events_transparent: false,
            start_mode,
            ..Default::default()
        },
        fonts: Vec::new(),
        default_font: Font::default(),
        default_text_size: Pixels(16.0),
        antialiasing: true,
        virtual_keyboard_support: None,
    })
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
    let reward_cards = rewards.iter().fold(row![].spacing(10), |row, reward| {
        row.push(reward_card(reward))
    });

    container(reward_cards)
        .padding(18)
        .width(Length::Shrink)
        .height(Length::Shrink)
        .style(|_theme| container::Style {
            background: Some(Color::from_rgba(0.03, 0.04, 0.05, 0.78).into()),
            border: iced::Border {
                color: Color::from_rgb(0.88, 0.72, 0.32),
                width: 2.0,
                radius: 4.0.into(),
            },
            text_color: Some(Color::WHITE),
            ..Default::default()
        })
        .into()
}

fn reward_card(reward: &RewardOverlayEntry) -> Element<'_, Message, Theme, Renderer> {
    let mut details = column![text(&reward.name).size(16)].spacing(4);

    if let Some(platinum) = reward.platinum {
        details = details.push(text(format!("{platinum} platinum")).size(14));
    }

    if let Some(ducats) = reward.ducats {
        details = details.push(text(format!("{ducats} ducats")).size(14));
    }

    if let Some(volume) = reward.volume {
        details = details.push(text(format!("{volume} sold recently")).size(13));
    }

    if reward.vaulted {
        details = details.push(text("Vaulted").size(13));
    }

    if let (Some(owned), Some(required)) = (reward.owned_count, reward.required_count) {
        details = details.push(text(format!("Owned {owned}/{required}")).size(13));
    }

    container(details)
        .padding(12)
        .width(Length::Fixed(180.0))
        .style(|_theme| container::Style {
            background: Some(Color::from_rgba(0.08, 0.09, 0.11, 0.92).into()),
            border: iced::Border {
                color: Color::from_rgb(0.30, 0.34, 0.38),
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: Some(Color::WHITE),
            ..Default::default()
        })
        .into()
}
