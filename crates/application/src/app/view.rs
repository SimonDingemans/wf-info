use iced::widget::{button, column, container, pick_list, row, scrollable, text};
use iced::{Element, Length};

use super::message::Message;
use super::state::Application;

impl Application {
    pub(super) fn view(&self) -> Element<'_, Message> {
        let detect_button = button("Show Monitor Info Overlays")
            .padding([10, 14])
            .on_press_maybe((!self.busy).then_some(Message::DetectMonitorInfo));

        let test_overlay_button = button("Draw Test Overlay")
            .padding([10, 14])
            .on_press(Message::DrawTestOverlay);
        let quit_overlays_button = button("Quit Debug Overlays")
            .padding([10, 14])
            .on_press(Message::QuitDebugOverlays);

        let selected = self.selected_monitor.clone();
        let monitor_picker = pick_list(
            self.monitors.as_slice(),
            selected,
            Message::SelectedMonitorChanged,
        )
        .placeholder("Active screen");

        let monitor_rows = self
            .monitors
            .iter()
            .fold(column![].spacing(8), |column, monitor| {
                let details = monitor
                    .info
                    .summary_lines()
                    .into_iter()
                    .fold(column![].spacing(2), |column, line| column.push(text(line)));

                column.push(container(details).padding(12).width(Length::Fill))
            });

        let content = column![
            text("wf-info").size(32),
            text("Basic application shell").size(18),
            row![detect_button, test_overlay_button, quit_overlays_button].spacing(12),
            row![text("Selected capture/overlay target:"), monitor_picker]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            text(&self.status),
            scrollable(monitor_rows).height(Length::Fill),
        ]
        .spacing(16)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
