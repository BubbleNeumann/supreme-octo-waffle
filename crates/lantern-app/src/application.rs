use iced::widget::{Id, operation, text_editor};
use iced::{Event, Size, Subscription, Task, event, keyboard, mouse};

const WINDOW_TITLE: &str = "Lantern";
const WINDOW_SIZE: Size = Size::new(960.0, 640.0);
const DEFAULT_EDITOR_FONT_SIZE: f32 = 16.0;
const MIN_EDITOR_FONT_SIZE: f32 = 10.0;
const MAX_EDITOR_FONT_SIZE: f32 = 32.0;
const FONT_ZOOM_STEP: f32 = 1.0;

pub(crate) fn run() -> iced::Result {
    iced::application(boot, update, crate::ui::view)
        .title(WINDOW_TITLE)
        .subscription(subscription)
        .window_size(WINDOW_SIZE)
        .centered()
        .exit_on_close_request(true)
        .run()
}

#[derive(Debug)]
pub(crate) struct Lantern {
    pub(crate) editor: text_editor::Content,
    pub(crate) editor_id: Id,
    pub(crate) editor_font_size: f32,
    modifiers: keyboard::Modifiers,
    pub(crate) sidebar_collapsed: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Edit(text_editor::Action),
    ModifiersChanged(keyboard::Modifiers),
    MouseWheelScrolled(mouse::ScrollDelta),
    ToggleSidebar,
}

fn boot() -> (Lantern, Task<Message>) {
    let editor_id = Id::unique();
    let focus_editor = operation::focus(editor_id.clone());

    (
        Lantern {
            editor: text_editor::Content::new(),
            editor_id,
            editor_font_size: DEFAULT_EDITOR_FONT_SIZE,
            modifiers: keyboard::Modifiers::default(),
            sidebar_collapsed: false,
        },
        focus_editor,
    )
}

fn update(lantern: &mut Lantern, message: Message) {
    match message {
        Message::Edit(action) => {
            let is_controlled_scroll =
                lantern.modifiers.control() && matches!(action, text_editor::Action::Scroll { .. });

            if !is_controlled_scroll {
                lantern.editor.perform(action);
            }
        }
        Message::ModifiersChanged(modifiers) => lantern.modifiers = modifiers,
        Message::MouseWheelScrolled(delta) => {
            if lantern.modifiers.control() {
                let vertical_delta = match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => y,
                };

                let zoom = FONT_ZOOM_STEP * vertical_delta.signum();
                lantern.editor_font_size = (lantern.editor_font_size + zoom)
                    .clamp(MIN_EDITOR_FONT_SIZE, MAX_EDITOR_FONT_SIZE);
            }
        }
        Message::ToggleSidebar => lantern.sidebar_collapsed = !lantern.sidebar_collapsed,
    }
}

fn subscription(_lantern: &Lantern) -> Subscription<Message> {
    event::listen_with(|event, _status, _window| match event {
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            Some(Message::MouseWheelScrolled(delta))
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_message_updates_the_editor_buffer() {
        let (mut lantern, _) = boot();

        update(
            &mut lantern,
            Message::Edit(text_editor::Action::Edit(text_editor::Edit::Insert('L'))),
        );

        assert_eq!(lantern.editor.text(), "L");
    }

    #[test]
    fn toggle_sidebar_message_changes_its_visibility() {
        let (mut lantern, _) = boot();

        update(&mut lantern, Message::ToggleSidebar);
        assert!(lantern.sidebar_collapsed);

        update(&mut lantern, Message::ToggleSidebar);
        assert!(!lantern.sidebar_collapsed);
    }

    #[test]
    fn control_and_mouse_wheel_adjust_the_editor_font_size() {
        let (mut lantern, _) = boot();
        update(
            &mut lantern,
            Message::ModifiersChanged(keyboard::Modifiers::CTRL),
        );

        update(
            &mut lantern,
            Message::MouseWheelScrolled(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
        );
        assert_eq!(lantern.editor_font_size, DEFAULT_EDITOR_FONT_SIZE + 1.0);

        update(
            &mut lantern,
            Message::MouseWheelScrolled(mouse::ScrollDelta::Pixels { x: 0.0, y: -5.0 }),
        );
        assert_eq!(lantern.editor_font_size, DEFAULT_EDITOR_FONT_SIZE);
    }

    #[test]
    fn editor_font_size_stays_within_readable_limits() {
        let (mut lantern, _) = boot();
        lantern.modifiers = keyboard::Modifiers::CTRL;
        lantern.editor_font_size = MAX_EDITOR_FONT_SIZE;

        update(
            &mut lantern,
            Message::MouseWheelScrolled(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
        );

        assert_eq!(lantern.editor_font_size, MAX_EDITOR_FONT_SIZE);
    }
}
