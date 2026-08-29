use super::style;
use crate::application::explorer::ExplorerRow;
use crate::application::{Lantern, Message};
use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Element, Fill, Length, alignment};
use std::path::Path;

const SIDEBAR_WIDTH: f32 = 240.0;
const COLLAPSED_SIDEBAR_WIDTH: f32 = 24.0;
const SIDEBAR_HEADER_HEIGHT: f32 = 32.0;
const INDENTATION_PER_LEVEL: f32 = 14.0;
const DISCLOSURE_WIDTH: f32 = 14.0;

pub(super) fn view(lantern: &Lantern) -> Element<'_, Message> {
    if lantern.sidebar_collapsed {
        return collapsed();
    }

    let content = if lantern.project.is_some() {
        project_content(lantern)
    } else {
        no_project_content(lantern)
    };

    let content = match &lantern.project_error {
        Some(error) => content.push(text(format!("Project error: {error}")).size(11)),
        None => content,
    };

    container(column![header(), scrollable(content).height(Fill)].spacing(10))
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Fill)
        .padding(18)
        .style(style::sidebar_background)
        .into()
}

fn collapsed<'a>() -> Element<'a, Message> {
    container(
        button("›")
            .width(Fill)
            .height(Length::Fixed(SIDEBAR_HEADER_HEIGHT))
            .padding([0, 2])
            .style(style::square_button)
            .on_press(Message::ToggleSidebar),
    )
    .width(Length::Fixed(COLLAPSED_SIDEBAR_WIDTH))
    .height(Fill)
    .padding(1)
    .style(style::sidebar_background)
    .into()
}

fn header<'a>() -> Element<'a, Message> {
    row![
        text("Project Explorer").size(18),
        space().width(Fill),
        button("‹")
            .height(Fill)
            .padding([0, 8])
            .style(style::square_button)
            .on_press(Message::ToggleSidebar),
    ]
    .height(Length::Fixed(SIDEBAR_HEADER_HEIGHT))
    .align_y(alignment::Vertical::Center)
    .into()
}

fn project_content(lantern: &Lantern) -> iced::widget::Column<'_, Message> {
    let mut content = column![].spacing(6);

    if let Some(project) = &lantern.project {
        content = content.push(text(project.display_name()).size(15));
    }

    if lantern.explorer.is_empty() {
        return content.push(text("Empty project").size(12));
    }

    for row in lantern.explorer.visible_rows() {
        content = content.push(entry_row(row, lantern.open_document.as_deref()));
    }

    content
}

fn no_project_content(lantern: &Lantern) -> iced::widget::Column<'_, Message> {
    let mut content = column![].spacing(6);

    content = content
        .push(text("No project open").size(13))
        .push(
            button("Open Folder")
                .width(Fill)
                .style(style::square_button)
                .on_press(Message::OpenProject),
        )
        .push(
            button("New Project")
                .width(Fill)
                .style(style::square_button)
                .on_press(Message::BeginCreateProject),
        );

    if !lantern.creating_project {
        return content;
    }

    let can_create = !lantern.new_project_name.trim().is_empty();
    let actions = row![
        button("Create")
            .style(style::square_button)
            .on_press_maybe(can_create.then_some(Message::ChooseProjectParent)),
        button("Cancel")
            .style(style::square_button)
            .on_press(Message::CancelCreateProject),
    ]
    .spacing(6);

    content
        .push(
            text_input("Project name", &lantern.new_project_name)
                .on_input(Message::NewProjectNameChanged)
                .padding(8)
                .style(style::borderless_text_input),
        )
        .push(actions)
        .push(text("Create will ask for the parent folder.").size(11))
}

fn entry_row<'a>(
    explorer_row: ExplorerRow<'a>,
    open_document: Option<&'a Path>,
) -> Element<'a, Message> {
    let entry = explorer_row.entry;
    let indentation = space().width(Length::Fixed(
        explorer_row.depth as f32 * INDENTATION_PER_LEVEL,
    ));

    if entry.is_directory() {
        let disclosure = if explorer_row.expanded { "▾" } else { "›" };

        return button(row![
            indentation,
            container(text(disclosure)).width(Length::Fixed(DISCLOSURE_WIDTH)),
            text(entry.name()).size(13),
        ])
        .width(Fill)
        .padding([3, 0])
        .style(style::tree_button)
        .on_press(Message::ToggleProjectDirectory(
            entry.relative_path().to_owned(),
        ))
        .into();
    }

    let selected = open_document == Some(entry.relative_path());

    button(row![
        indentation,
        space().width(Length::Fixed(DISCLOSURE_WIDTH)),
        text(entry.name()).size(13),
    ])
    .width(Fill)
    .padding([3, 0])
    .style(move |theme, status| style::file_button(theme, status, selected))
    .on_press(Message::OpenDocument(entry.relative_path().to_owned()))
    .into()
}
