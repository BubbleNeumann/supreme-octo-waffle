use super::style;
use super::text_editor;
use crate::application::{Lantern, Message};
use iced::widget::{container, space};
use iced::{Color, Element, Fill};

pub(super) fn view(lantern: &Lantern) -> Element<'_, Message> {
    container(if lantern.accepts_writing() {
        editor(lantern)
    } else {
        // With no project there is nothing to write into, so the pane holds no
        // widget rather than a disabled one: nothing to click, nothing to
        // focus, no caret, no invitation to write, and a pointer that keeps
        // whatever shape it already had instead of being refused.
        space().width(Fill).height(Fill).into()
    })
    .width(Fill)
    .height(Fill)
    .padding(18)
    .style(move |_theme| editor_pane(lantern.editor_redraw_epoch))
    .into()
}

fn editor(lantern: &Lantern) -> Element<'_, Message> {
    text_editor::TextEditor::new(&lantern.editor)
        .id(lantern.editor_id.clone())
        .font(crate::application::EDITOR_FONT)
        .placeholder("Start writing...")
        .on_action(Message::Edit)
        .size(lantern.editor_font_size)
        .height(Fill)
        .padding(16)
        .style(style::borderless_editor)
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
