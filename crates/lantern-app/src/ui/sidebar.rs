use super::style;
use crate::application::explorer::ExplorerRow;
use crate::application::{HoveredEntry, Lantern, Message};
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, space, text, text_input,
};
use iced::{Element, Fill, Length, alignment};
use std::borrow::Cow;
use std::path::Path;

const SIDEBAR_WIDTH: f32 = 240.0;
const COLLAPSED_SIDEBAR_WIDTH: f32 = 24.0;
const SIDEBAR_HEADER_HEIGHT: f32 = 32.0;
const INDENTATION_PER_LEVEL: f32 = 14.0;
const DISCLOSURE_WIDTH: f32 = 14.0;
/// How deep into a document row's top edge means "before this one".
///
/// A row is a little over twenty pixels tall, so this is roughly its upper
/// third. It is deliberately less than half: most of a row means "after this
/// one", so that a pointer crossing rows on its way down the tree does not read
/// as an insertion above each one it passes.
const INSERTION_BAND: f32 = 8.0;
/// The thickness of the line drawn where a dragged document would land.
const INSERTION_LINE_HEIGHT: f32 = 2.0;
/// Follows the open document's name while it holds edits that are not on disk.
const UNSAVED_MARKER: &str = " \u{2022}";

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

    let content = match &lantern.theme_error {
        Some(error) => content.push(text(format!("Theme error: {error}")).size(11)),
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

    content = content.push(new_document(lantern));

    if lantern.explorer.is_empty() {
        return content.push(text("Empty project").size(12));
    }

    let mut tree = column![].spacing(6);
    let dragging = lantern.dragged_document.is_some();

    for row in lantern.explorer.visible_rows() {
        let drop_target = dragging
            && lantern.hovered_entry
                == Some(HoveredEntry::Directory(
                    row.entry.relative_path().to_owned(),
                ));
        let insertion = insertion_against(lantern, row.entry.relative_path());

        if insertion == Some(true) {
            tree = tree.push(insertion_line());
        }

        tree = tree.push(entry_row(
            row,
            lantern.open_document_path(),
            lantern.unsaved_edits,
            drop_target,
            dragging,
        ));

        if insertion == Some(false) {
            tree = tree.push(insertion_line());
        }
    }

    // Each row reports the pointer arriving, and the tree as a whole reports it
    // leaving. Rows report their own departures in the order they are drawn,
    // which for a pointer travelling upwards would clear the row it had just
    // arrived at; the tree only ever clears when the pointer is outside it.
    content.push(mouse_area(tree).on_exit(Message::EntryHovered(None)))
}

/// The control that adds a document, and the name field once it is asked for.
///
/// It is drawn only as part of a project's content, because a document created
/// with no project open would have nowhere to be written.
fn new_document(lantern: &Lantern) -> iced::widget::Column<'_, Message> {
    let content = column![
        button("New Document")
            .width(Fill)
            .style(style::square_button)
            .on_press(Message::BeginCreateDocument),
    ]
    .spacing(6);

    if !lantern.creating_document {
        return content;
    }

    let can_create = !lantern.new_document_name.trim().is_empty();
    let actions = row![
        button("Create")
            .style(style::square_button)
            .on_press_maybe(can_create.then_some(Message::CreateDocument)),
        button("Cancel")
            .style(style::square_button)
            .on_press(Message::CancelCreateDocument),
    ]
    .spacing(6);

    content
        .push(
            text_input("Document name", &lantern.new_document_name)
                .on_input(Message::NewDocumentNameChanged)
                .on_submit_maybe(can_create.then_some(Message::CreateDocument))
                .padding(8)
                .style(style::borderless_text_input),
        )
        .push(actions)
        .push(
            text(format!(
                "Created in {}, as Markdown unless named otherwise.",
                lantern.new_document_directory().display()
            ))
            .size(11),
        )
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

/// Returns whether a dragged document would land above or below a row.
///
/// `None` while nothing is being dragged, while the pointer is over another
/// row, and over the dragged document's own row, which asks for nothing.
fn insertion_against(lantern: &Lantern, row: &Path) -> Option<bool> {
    let dragged = lantern.dragged_document.as_deref()?;

    match &lantern.hovered_entry {
        Some(HoveredEntry::Document {
            relative_path,
            above,
        }) if relative_path == row && relative_path != dragged => Some(*above),
        _ => None,
    }
}

/// Draws the line a dragged document would land on.
fn insertion_line<'a>() -> Element<'a, Message> {
    container(space().height(Length::Fixed(INSERTION_LINE_HEIGHT)))
        .width(Fill)
        .style(style::insertion_line)
        .into()
}

fn entry_row<'a>(
    explorer_row: ExplorerRow<'a>,
    open_document: Option<&'a Path>,
    unsaved_edits: bool,
    drop_target: bool,
    dragging: bool,
) -> Element<'a, Message> {
    let entry = explorer_row.entry;
    let indentation = space().width(Length::Fixed(
        explorer_row.depth as f32 * INDENTATION_PER_LEVEL,
    ));

    if entry.is_directory() {
        let disclosure = if explorer_row.expanded { "▾" } else { "›" };

        return mouse_area(
            button(row![
                indentation,
                container(text(disclosure)).width(Length::Fixed(DISCLOSURE_WIDTH)),
                text(entry.name()).size(13),
            ])
            .width(Fill)
            .padding([3, 0])
            .style(move |theme, status| style::tree_button(theme, status, drop_target))
            .on_press(Message::ToggleProjectDirectory(
                entry.relative_path().to_owned(),
            )),
        )
        .on_enter(Message::EntryHovered(Some(HoveredEntry::Directory(
            entry.relative_path().to_owned(),
        ))))
        .into();
    }

    let selected = open_document == Some(entry.relative_path());
    let title = drawn_name(entry.name());
    let name: Cow<'a, str> = if selected && unsaved_edits {
        format!("{title}{UNSAVED_MARKER}").into()
    } else {
        title.into()
    };

    let hovered = mouse_area(
        button(row![
            indentation,
            space().width(Length::Fixed(DISCLOSURE_WIDTH)),
            text(name).size(13),
        ])
        .width(Fill)
        .padding([3, 0])
        .style(move |theme, status| style::file_button(theme, status, selected))
        .on_press(Message::OpenDocument(entry.relative_path().to_owned())),
    )
    .on_enter(Message::EntryHovered(Some(HoveredEntry::Document {
        relative_path: entry.relative_path().to_owned(),
        above: false,
    })));

    // Where in the row the pointer is only matters while a document is being
    // carried, and asking for it costs a message on every mouse move, so the
    // row reports its position only while there is a drag to place.
    if !dragging {
        return hovered.into();
    }

    hovered
        .on_move(|point| {
            Message::EntryHovered(Some(HoveredEntry::Document {
                relative_path: entry.relative_path().to_owned(),
                above: point.y < INSERTION_BAND,
            }))
        })
        .into()
}

/// Returns the name a file is drawn under: its own, without the extension.
///
/// The explorer reads as the manuscript's list of titles rather than as a
/// directory listing, and which of the editable formats a document happens to
/// be saved in is Lantern's business rather than the author's. Only the drawn
/// name loses the extension; the entry's path keeps every byte the operating
/// system reported, and is what opening resolves.
///
/// A name that is all extension - `.gitignore` - keeps it, because its stem is
/// the whole name.
fn drawn_name(entry_name: &str) -> &str {
    Path::new(entry_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(entry_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_document_is_drawn_under_its_title_alone() {
        assert_eq!(drawn_name("Chapter One.md"), "Chapter One");
        assert_eq!(drawn_name("sources.txt"), "sources");
        assert_eq!(drawn_name("outline.markdown"), "outline");
    }

    #[test]
    fn a_title_holding_a_period_keeps_everything_but_the_extension() {
        assert_eq!(drawn_name("Mrs. Dalloway.md"), "Mrs. Dalloway");
    }

    #[test]
    fn a_name_that_is_all_extension_is_drawn_whole() {
        assert_eq!(drawn_name(".gitignore"), ".gitignore");
    }

    #[test]
    fn a_name_carrying_no_extension_is_drawn_whole() {
        assert_eq!(drawn_name("LICENSE"), "LICENSE");
    }
}
