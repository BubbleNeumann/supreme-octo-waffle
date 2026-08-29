mod editor;
mod sidebar;
mod style;

use crate::application::{Lantern, Message};
use iced::widget::row;
use iced::{Element, Fill};

pub(crate) fn view(lantern: &Lantern) -> Element<'_, Message> {
    row![sidebar::view(lantern), editor::view(lantern)]
        .width(Fill)
        .height(Fill)
        .into()
}
