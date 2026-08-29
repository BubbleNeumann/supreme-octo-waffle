pub(crate) mod explorer;

use explorer::Explorer;
use iced::widget::{Id, operation, text_editor};
use iced::{Event, Size, Subscription, Task, event, keyboard};
use lantern_service::{FsProjectService, Project};
use std::path::{Path, PathBuf};

const WINDOW_TITLE: &str = "Lantern";
const WINDOW_SIZE: Size = Size::new(960.0, 640.0);
const DEFAULT_EDITOR_FONT_SIZE: f32 = 16.0;
const MIN_EDITOR_FONT_SIZE: f32 = 10.0;
const MAX_EDITOR_FONT_SIZE: f32 = 32.0;
const FONT_ZOOM_STEP: f32 = 1.0;
/// Drawn when no theme file can be read, so the window is never unstyled.
const FALLBACK_THEME: iced::Theme = iced::Theme::Dark;

pub(crate) fn run() -> iced::Result {
    iced::application(boot, update, crate::ui::view)
        .title(WINDOW_TITLE)
        .subscription(subscription)
        .theme(theme)
        .window_size(WINDOW_SIZE)
        .centered()
        .exit_on_close_request(true)
        .run()
}

#[derive(Debug)]
pub(crate) struct Lantern {
    project_service: FsProjectService,
    pub(crate) project: Option<Project>,
    pub(crate) explorer: Explorer,
    pub(crate) open_document: Option<PathBuf>,
    pub(crate) project_error: Option<String>,
    pub(crate) creating_project: bool,
    pub(crate) new_project_name: String,
    pub(crate) editor: text_editor::Content,
    pub(crate) editor_id: Id,
    pub(crate) editor_font_size: f32,
    pub(crate) editor_redraw_epoch: bool,
    modifiers: keyboard::Modifiers,
    pub(crate) sidebar_collapsed: bool,
    interface_theme: iced::Theme,
    pub(crate) theme_error: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    BeginCreateProject,
    CancelCreateProject,
    ChooseProjectParent,
    Edit(text_editor::Action),
    ModifiersChanged(keyboard::Modifiers),
    NewProjectNameChanged(String),
    OpenDocument(PathBuf),
    OpenProject,
    OpenProjectFolderPicked(Option<PathBuf>),
    ProjectParentPicked(Option<PathBuf>),
    ToggleProjectDirectory(PathBuf),
    ToggleSidebar,
}

fn boot() -> (Lantern, Task<Message>) {
    let editor_id = Id::unique();
    let focus_editor = operation::focus(editor_id.clone());
    let (interface_theme, theme_error) =
        match crate::theme::service().theme(crate::theme::DEFAULT_THEME) {
            Ok(theme) => (crate::theme::to_iced(&theme), None),
            // A missing or malformed theme must not stop Lantern from opening,
            // so fall back to a built-in palette and say why in the sidebar.
            Err(error) => (FALLBACK_THEME, Some(error.to_string())),
        };

    (
        Lantern {
            project_service: FsProjectService::filesystem(),
            project: None,
            explorer: Explorer::new(),
            open_document: None,
            project_error: None,
            creating_project: false,
            new_project_name: String::new(),
            editor: text_editor::Content::new(),
            editor_id,
            editor_font_size: DEFAULT_EDITOR_FONT_SIZE,
            editor_redraw_epoch: false,
            modifiers: keyboard::Modifiers::default(),
            sidebar_collapsed: false,
            interface_theme,
            theme_error,
        },
        focus_editor,
    )
}

fn update(lantern: &mut Lantern, message: Message) -> Task<Message> {
    match message {
        Message::BeginCreateProject => {
            lantern.creating_project = true;
            lantern.new_project_name.clear();
            lantern.project_error = None;
        }
        Message::CancelCreateProject => {
            lantern.creating_project = false;
            lantern.new_project_name.clear();
            lantern.project_error = None;
        }
        Message::ChooseProjectParent => {
            lantern.project_error = None;

            return Task::perform(
                pick_folder("Choose where to create the Lantern project"),
                Message::ProjectParentPicked,
            );
        }
        Message::Edit(action) => edit_document(lantern, action),
        Message::ModifiersChanged(modifiers) => lantern.modifiers = modifiers,
        Message::NewProjectNameChanged(name) => lantern.new_project_name = name,
        Message::OpenDocument(relative_path) => {
            if open_document(lantern, &relative_path) {
                return operation::focus(lantern.editor_id.clone());
            }
        }
        Message::OpenProject => {
            lantern.project_error = None;

            return Task::perform(
                pick_folder("Open an existing folder as a Lantern project"),
                Message::OpenProjectFolderPicked,
            );
        }
        Message::OpenProjectFolderPicked(Some(root)) => {
            let result = lantern.project_service.open_project(&root);
            apply_project_result(lantern, result);
        }
        Message::OpenProjectFolderPicked(None) => {}
        Message::ProjectParentPicked(Some(parent)) => {
            let result = lantern
                .project_service
                .create_project(&parent, lantern.new_project_name.clone());
            apply_project_result(lantern, result);
        }
        Message::ProjectParentPicked(None) => {}
        Message::ToggleProjectDirectory(relative_path) => {
            toggle_project_directory(lantern, relative_path);
        }
        Message::ToggleSidebar => lantern.sidebar_collapsed = !lantern.sidebar_collapsed,
    }

    Task::none()
}

async fn pick_folder(title: &'static str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .pick_folder()
        .await
        .map(|folder| folder.path().to_owned())
}

fn edit_document(lantern: &mut Lantern, action: text_editor::Action) {
    let text_editor::Action::Scroll { lines } = action else {
        lantern.editor.perform(action);
        return;
    };

    if lines == 0 {
        return;
    }

    if lantern.modifiers.control() {
        let zoom = -FONT_ZOOM_STEP * (lines as f32).signum();
        lantern.editor_font_size =
            (lantern.editor_font_size + zoom).clamp(MIN_EDITOR_FONT_SIZE, MAX_EDITOR_FONT_SIZE);
    } else {
        lantern
            .editor
            .perform(text_editor::Action::Scroll { lines });
    }

    // The tiny-skia compositor uses incremental damage tracking. An invisible,
    // changing pane primitive makes the editor viewport's damage explicit and
    // prevents stale pixels at its bottom edge.
    lantern.editor_redraw_epoch = !lantern.editor_redraw_epoch;
}

fn apply_project_result(
    lantern: &mut Lantern,
    result: Result<Project, lantern_service::ProjectServiceError>,
) {
    match result {
        Ok(project) => {
            let root_entries = lantern
                .project_service
                .list_directory(&project, Path::new(""));

            lantern.project = Some(project);
            lantern.open_document = None;
            lantern.editor = text_editor::Content::new();

            match root_entries {
                Ok(entries) => {
                    lantern.explorer.reset(entries);
                    lantern.project_error = None;
                }
                Err(error) => {
                    lantern.explorer.clear();
                    lantern.project_error = Some(error.to_string());
                }
            }

            lantern.creating_project = false;
            lantern.new_project_name.clear();
        }
        Err(error) => lantern.project_error = Some(error.to_string()),
    }
}

fn open_document(lantern: &mut Lantern, relative_path: &Path) -> bool {
    let Some(project) = lantern.project.as_ref() else {
        return false;
    };

    match lantern
        .project_service
        .open_document(project, relative_path)
    {
        Ok(document) => {
            lantern.editor = text_editor::Content::with_text(document.content());
            lantern.open_document = Some(document.relative_path().to_owned());
            lantern.project_error = None;
            true
        }
        Err(error) => {
            lantern.project_error = Some(error.to_string());
            false
        }
    }
}

fn toggle_project_directory(lantern: &mut Lantern, relative_path: PathBuf) {
    if lantern.explorer.is_expanded(&relative_path) {
        lantern.explorer.collapse(&relative_path);
        return;
    }

    lantern.explorer.expand(relative_path);
    load_visible_directories(lantern);
}

/// Reads every expanded directory the explorer shows without a listing.
///
/// Collapsing releases the listings underneath a directory, so re-expanding it
/// reads back the subtree that its surviving expansion markers describe.
fn load_visible_directories(lantern: &mut Lantern) {
    let Some(project) = lantern.project.as_ref() else {
        return;
    };

    while let Some(directory) = lantern.explorer.next_unlisted_directory() {
        match lantern.project_service.list_directory(project, &directory) {
            Ok(entries) => {
                lantern.explorer.insert_listing(directory, entries);
                lantern.project_error = None;
            }
            Err(error) => {
                // Leaving it expanded would ask for the same failing listing
                // again on every later expansion.
                lantern.explorer.collapse(&directory);
                lantern.project_error = Some(error.to_string());
                return;
            }
        }
    }
}

fn theme(lantern: &Lantern) -> iced::Theme {
    lantern.interface_theme.clone()
}

fn subscription(_lantern: &Lantern) -> Subscription<Message> {
    event::listen_with(|event, _status, _window| match event {
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests;
