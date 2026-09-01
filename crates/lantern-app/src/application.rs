pub(crate) mod explorer;

use explorer::Explorer;
use iced::futures::Stream;
use iced::widget::{Id, operation, text_editor};
use iced::{Event, Size, Subscription, Task, event, keyboard, stream};
use lantern_service::{Document, FsProjectService, Project};
use std::path::{Path, PathBuf};
use std::time::Duration;

const WINDOW_TITLE: &str = "Lantern";
const WINDOW_SIZE: Size = Size::new(960.0, 640.0);
const DEFAULT_EDITOR_FONT_SIZE: f32 = 16.0;
const MIN_EDITOR_FONT_SIZE: f32 = 10.0;
const MAX_EDITOR_FONT_SIZE: f32 = 32.0;
const FONT_ZOOM_STEP: f32 = 1.0;
/// How long an edit may stay unwritten before autosave stores it.
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(10);
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
    pub(crate) open_document: Option<Document>,
    pub(crate) unsaved_edits: bool,
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

impl Lantern {
    /// Returns the project-relative path of the document being edited.
    pub(crate) fn open_document_path(&self) -> Option<&Path> {
        self.open_document.as_ref().map(Document::relative_path)
    }
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
    SaveDocument,
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
            unsaved_edits: false,
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
        Message::SaveDocument => save_open_document(lantern),
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
        lantern.unsaved_edits |= action.is_edit();
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
            // The document being edited belongs to the outgoing project, so it
            // has to reach disk before that project is replaced.
            save_open_document(lantern);

            let root_entries = lantern
                .project_service
                .list_directory(&project, Path::new(""));

            lantern.project = Some(project);
            lantern.open_document = None;
            lantern.unsaved_edits = false;
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
    // Whatever is in the editor is about to be replaced, so it has to reach
    // disk first; opening another document must not discard unsaved work.
    save_open_document(lantern);

    let Some(project) = lantern.project.as_ref() else {
        return false;
    };

    match lantern
        .project_service
        .open_document(project, relative_path)
    {
        Ok(document) => {
            lantern.editor = text_editor::Content::with_text(document.content());
            lantern.open_document = Some(document);
            lantern.unsaved_edits = false;
            lantern.project_error = None;
            true
        }
        Err(error) => {
            lantern.project_error = Some(error.to_string());
            false
        }
    }
}

/// Writes the editor's text over the open document when the two differ.
///
/// Both Ctrl+S and autosave arrive here, so an explicit save and an automatic
/// one leave exactly the same file behind. A buffer that matches the document
/// is left alone rather than rewritten.
fn save_open_document(lantern: &mut Lantern) {
    if !lantern.unsaved_edits {
        return;
    }

    let Some(project) = lantern.project.as_ref() else {
        return;
    };
    let Some(document) = lantern.open_document.as_mut() else {
        return;
    };

    let content = lantern.editor.text();

    // Editing and then undoing leaves the buffer marked, but identical to the
    // file; there is nothing to write.
    if !document.differs_from(&content) {
        lantern.unsaved_edits = false;
        return;
    }

    match lantern
        .project_service
        .save_document(project, document, &content)
    {
        Ok(()) => {
            lantern.unsaved_edits = false;
            lantern.project_error = None;
        }
        Err(error) => lantern.project_error = Some(error.to_string()),
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

fn subscription(lantern: &Lantern) -> Subscription<Message> {
    let mut subscriptions = vec![event::listen_with(|event, _status, _window| match event {
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        }) => {
            // `command` is Ctrl everywhere Lantern runs except macOS, where the
            // same shortcut is written with the Command key. The key is read
            // the way the editor reads its own shortcuts, so that Ctrl+S stays
            // Ctrl+S on a keyboard layout that is not Latin.
            (modifiers.command() && key.to_latin(physical_key) == Some('s'))
                .then_some(Message::SaveDocument)
        }
        _ => None,
    })];

    // Nothing can be autosaved without an open document, and the interval
    // starts over with the next one that is opened.
    if lantern.open_document.is_some() {
        subscriptions.push(Subscription::run(autosave_ticks));
    }

    Subscription::batch(subscriptions)
}

/// Asks for a save every [`AUTOSAVE_INTERVAL`] for as long as it is listened to.
fn autosave_ticks() -> impl Stream<Item = Message> {
    save_requests(AUTOSAVE_INTERVAL)
}

/// Produces a save request every `interval` until the stream is dropped.
///
/// Lantern runs on the futures thread pool, which offers no timer, so the
/// interval is kept by a thread that parks between ticks rather than by an
/// async runtime the application would otherwise not need.
fn save_requests(interval: Duration) -> impl Stream<Item = Message> {
    stream::channel(1, async move |mut ticks| {
        // Without a ticker there is no autosave; Ctrl+S still works, and the
        // editor is no worse off than it would be for refusing to open.
        let _ = std::thread::Builder::new()
            .name("lantern-autosave".to_owned())
            .spawn(move || {
                loop {
                    std::thread::sleep(interval);

                    match ticks.try_send(Message::SaveDocument) {
                        Ok(()) => {}
                        // A tick is already waiting to be handled; one is enough.
                        Err(error) if error.is_full() => {}
                        // The subscription was dropped, so this thread is done.
                        Err(_) => return,
                    }
                }
            });
    })
}

#[cfg(test)]
mod tests;
