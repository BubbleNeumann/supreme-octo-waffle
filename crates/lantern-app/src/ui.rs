use crate::application::{Lantern, Message};
use iced::widget::{button, column, container, row, space, text, text_editor};
use iced::{Background, Border, Element, Fill, Length, Theme, alignment};

const SIDEBAR_WIDTH: f32 = 240.0;
const COLLAPSED_SIDEBAR_WIDTH: f32 = 28.0;
const SIDEBAR_HEADER_HEIGHT: f32 = 32.0;

pub(crate) fn view(lantern: &Lantern) -> Element<'_, Message> {
    row![sidebar(lantern.sidebar_collapsed), editor(lantern)]
        .width(Fill)
        .height(Fill)
        .into()
}

fn sidebar<'a>(collapsed: bool) -> Element<'a, Message> {
    if collapsed {
        return container(
            button("›")
                .width(Fill)
                .height(Length::Fixed(SIDEBAR_HEADER_HEIGHT))
                .padding([0, 4])
                .style(square_button)
                .on_press(Message::ToggleSidebar),
        )
        .width(Length::Fixed(COLLAPSED_SIDEBAR_WIDTH))
        .height(Fill)
        .padding(2)
        .style(square_sidebar)
        .into();
    }

    let header = row![
        text("Project Explorer").size(18),
        space().width(Fill),
        button("‹")
            .height(Fill)
            .padding([0, 8])
            .style(square_button)
            .on_press(Message::ToggleSidebar),
    ]
    .height(Length::Fixed(SIDEBAR_HEADER_HEIGHT))
    .align_y(alignment::Vertical::Center);

    container(column![header, text("No project open").size(13)].spacing(8))
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Fill)
        .padding(18)
        .style(square_sidebar)
        .into()
}

fn editor(lantern: &Lantern) -> Element<'_, Message> {
    container(
        text_editor(&lantern.editor)
            .id(lantern.editor_id.clone())
            .placeholder("Start writing...")
            .on_action(Message::Edit)
            .size(lantern.editor_font_size)
            .height(Fill)
            .padding(16)
            .style(borderless_editor),
    )
    .width(Fill)
    .height(Fill)
    .padding(18)
    .into()
}

fn square_sidebar(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: palette.background.weak.color,
        },
        ..container::Style::default()
    }
}

fn square_button(theme: &Theme, status: button::Status) -> button::Style {
    let style = button::subtle(theme, status);

    button::Style {
        border: Border {
            radius: 0.0.into(),
            ..style.border
        },
        ..style
    }
}

fn borderless_editor(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let palette = theme.extended_palette();

    text_editor::Style {
        background: Background::Color(palette.background.base.color),
        border: Border::default(),
        placeholder: palette.secondary.base.color,
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
    }
}
