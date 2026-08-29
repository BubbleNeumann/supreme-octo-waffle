use iced::widget::{button, container, text_editor, text_input};
use iced::{Background, Border, Theme};

pub(super) fn sidebar_background(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        ..container::Style::default()
    }
}

pub(super) fn square_button(theme: &Theme, status: button::Status) -> button::Style {
    let style = button::subtle(theme, status);

    button::Style {
        border: Border {
            radius: 0.0.into(),
            ..style.border
        },
        ..style
    }
}

pub(super) fn tree_button(theme: &Theme, status: button::Status) -> button::Style {
    let style = button::subtle(theme, status);

    button::Style {
        border: Border::default(),
        ..style
    }
}

pub(super) fn file_button(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let palette = theme.extended_palette();
    let mut style = button::subtle(theme, status);
    style.border = Border::default();

    if selected {
        style.background = Some(palette.primary.weak.color.into());
        style.text_color = palette.primary.weak.text;
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = Some(palette.background.weak.color.into());
        style.text_color = palette.background.weak.text;
    }

    style
}

pub(super) fn borderless_text_input(
    theme: &Theme,
    _status: text_input::Status,
) -> text_input::Style {
    let palette = theme.extended_palette();

    text_input::Style {
        background: Background::Color(palette.background.weak.color),
        border: Border::default(),
        icon: palette.background.weak.text,
        placeholder: palette.secondary.base.color,
        value: palette.background.weak.text,
        selection: palette.primary.weak.color,
    }
}

pub(super) fn borderless_editor(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let palette = theme.extended_palette();

    text_editor::Style {
        background: Background::Color(palette.background.base.color),
        border: Border::default(),
        placeholder: palette.secondary.base.color,
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
    }
}
