use crate::application::{Lantern, Message, ProjectTreeRow};
use iced::widget::{
    button, column, container, row, scrollable, space, text, text_editor, text_input,
};
use iced::{Background, Border, Color, Element, Fill, Length, Theme, alignment};

const SIDEBAR_WIDTH: f32 = 240.0;
const COLLAPSED_SIDEBAR_WIDTH: f32 = 24.0;
const SIDEBAR_HEADER_HEIGHT: f32 = 32.0;

pub(crate) fn view(lantern: &Lantern) -> Element<'_, Message> {
    row![sidebar(lantern), editor(lantern)]
        .width(Fill)
        .height(Fill)
        .into()
}

fn sidebar(lantern: &Lantern) -> Element<'_, Message> {
    if lantern.sidebar_collapsed {
        return container(
            button("›")
                .width(Fill)
                .height(Length::Fixed(SIDEBAR_HEADER_HEIGHT))
                .padding([0, 2])
                .style(square_button)
                .on_press(Message::ToggleSidebar),
        )
        .width(Length::Fixed(COLLAPSED_SIDEBAR_WIDTH))
        .height(Fill)
        .padding(1)
        .style(sidebar_background)
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

    let mut content = column![].spacing(6);

    if let Some(project) = &lantern.project {
        content = content.push(text(project.display_name()).size(15));

        if lantern.project_tree.is_empty() {
            content = content.push(text("Empty project").size(12));
        } else {
            for tree_row in &lantern.project_tree {
                content =
                    content.push(project_tree_row(tree_row, lantern.open_document.as_deref()));
            }
        }
    } else {
        content = content
            .push(text("No project open").size(13))
            .push(
                button("Open Folder")
                    .width(Fill)
                    .style(square_button)
                    .on_press(Message::OpenProject),
            )
            .push(
                button("New Project")
                    .width(Fill)
                    .style(square_button)
                    .on_press(Message::BeginCreateProject),
            );

        if lantern.creating_project {
            let can_create = !lantern.new_project_name.trim().is_empty();
            let actions = row![
                button("Create")
                    .style(square_button)
                    .on_press_maybe(can_create.then_some(Message::ChooseProjectParent)),
                button("Cancel")
                    .style(square_button)
                    .on_press(Message::CancelCreateProject),
            ]
            .spacing(6);

            content = content
                .push(
                    text_input("Project name", &lantern.new_project_name)
                        .on_input(Message::NewProjectNameChanged)
                        .padding(8)
                        .style(borderless_text_input),
                )
                .push(actions)
                .push(text("Create will ask for the parent folder.").size(11));
        }
    }

    if let Some(error) = &lantern.project_error {
        content = content.push(text(format!("Project error: {error}")).size(11));
    }

    container(column![header, scrollable(content).height(Fill)].spacing(10))
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Fill)
        .padding(18)
        .style(sidebar_background)
        .into()
}

fn project_tree_row<'a>(
    tree_row: &'a ProjectTreeRow,
    open_document: Option<&'a std::path::Path>,
) -> Element<'a, Message> {
    let indentation = space().width(Length::Fixed(tree_row.depth as f32 * 14.0));

    if tree_row.entry.is_directory() {
        let disclosure = if tree_row.expanded { "▾" } else { "›" };

        button(row![
            indentation,
            container(text(disclosure)).width(Length::Fixed(14.0)),
            text(tree_row.entry.name()).size(13),
        ])
        .width(Fill)
        .padding([3, 0])
        .style(tree_button)
        .on_press(Message::ToggleProjectDirectory(
            tree_row.entry.relative_path().to_owned(),
        ))
        .into()
    } else {
        let selected = open_document == Some(tree_row.entry.relative_path());

        button(row![
            indentation,
            space().width(Length::Fixed(14.0)),
            text(tree_row.entry.name()).size(13),
        ])
        .width(Fill)
        .padding([3, 0])
        .style(move |theme, status| file_button(theme, status, selected))
        .on_press(Message::OpenDocument(
            tree_row.entry.relative_path().to_owned(),
        ))
        .into()
    }
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

fn sidebar_background(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
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

fn tree_button(theme: &Theme, status: button::Status) -> button::Style {
    let style = button::subtle(theme, status);

    button::Style {
        border: Border::default(),
        ..style
    }
}

fn file_button(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
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

fn borderless_text_input(theme: &Theme, _status: text_input::Status) -> text_input::Style {
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
