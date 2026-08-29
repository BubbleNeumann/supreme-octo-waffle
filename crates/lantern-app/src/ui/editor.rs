use super::style;
use crate::application::{Lantern, Message};
use iced::widget::{container, text_editor};
use iced::{Color, Element, Fill};

pub(super) fn view(lantern: &Lantern) -> Element<'_, Message> {
    container(
        text_editor(&lantern.editor)
            .id(lantern.editor_id.clone())
            .placeholder("Start writing...")
            .on_action(Message::Edit)
            .size(lantern.editor_font_size)
            .height(Fill)
            .padding(16)
            .style(style::borderless_editor),
    )
    .width(Fill)
    .height(Fill)
    .padding(18)
    .style(move |_theme| editor_pane(lantern.editor_redraw_epoch))
    .into()
}

fn editor_pane(redraw_epoch: bool) -> container::Style {
    // Both colors are fully transparent. Alternating their RGB payload changes
    // the render primitive without changing its visible result.
    let invisible_damage_marker = if redraw_epoch {
        Color::from_rgba(1.0, 0.0, 0.0, 0.0)
    } else {
        Color::TRANSPARENT
    };

    container::Style {
        background: Some(invisible_damage_marker.into()),
        ..container::Style::default()
    }
}
