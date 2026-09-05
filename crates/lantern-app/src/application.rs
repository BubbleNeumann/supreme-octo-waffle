pub(crate) mod explorer;

use crate::ui::text_editor;
use explorer::Explorer;
use iced::futures::Stream;
use iced::widget::{Id, operation};
use iced::{Event, Size, Subscription, Task, event, keyboard, mouse, stream};
use lantern_service::{
    DEFAULT_DOCUMENT_DIRECTORY, Document, FsProjectService, Project, is_chapter, scene_directory,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

const WINDOW_TITLE: &str = "Lantern";
const WINDOW_SIZE: Size = Size::new(960.0, 640.0);
/// The typeface documents are written and read in.
///
/// IBM Plex Mono is monospaced but drawn for running text rather than only for
/// code, so a manuscript in it reads as prose. It is carried in the binary
/// rather than looked for on the system, because a writing application that
/// falls back to whatever monospace a machine happens to have is a writing
/// application that looks different on every machine.
pub(crate) const EDITOR_FONT: iced::Font = iced::Font::with_name("IBM Plex Mono");
/// The bytes of [`EDITOR_FONT`], registered with Iced before the window opens.
pub(crate) const EDITOR_FONT_BYTES: &[u8] =
    include_bytes!("../../../fonts/IBMPlexMono-Regular.ttf");
const DEFAULT_EDITOR_FONT_SIZE: f32 = 16.0;
const MIN_EDITOR_FONT_SIZE: f32 = 10.0;
const MAX_EDITOR_FONT_SIZE: f32 = 32.0;
const FONT_ZOOM_STEP: f32 = 1.0;
/// How long an edit may stay unwritten before autosave stores it.
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(10);
/// The mark the window carries into the taskbar and the window list.
///
/// One image serves every size the system asks for, and it is the largest of
/// the drawn marks that is still square, so that a system scaling it for a
/// small slot is always scaling down. Iced wants pixels rather than a file, so
/// this is decoded on the way in.
const WINDOW_ICON_BYTES: &[u8] = include_bytes!("../../../icons/lantern-logo-taskbar.png");
/// Drawn when no theme file can be read, so the window is never unstyled.
const FALLBACK_THEME: iced::Theme = iced::Theme::Dark;

pub(crate) fn run() -> iced::Result {
    iced::application(boot, update, crate::ui::view)
        .font(EDITOR_FONT_BYTES)
        .title(WINDOW_TITLE)
        .subscription(subscription)
        .theme(theme)
        // Settings first: the calls below fold into these, while this one
        // would replace whatever they had set.
        .window(iced::window::Settings {
            icon: window_icon(),
            ..iced::window::Settings::default()
        })
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
    pub(crate) creating_document: bool,
    pub(crate) new_document_name: String,
    pub(crate) hovered_entry: Option<HoveredEntry>,
    pub(crate) dragged_document: Option<PathBuf>,
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
    /// Returns whether the editor should accept writing.
    ///
    /// Text typed with no project open has nowhere to be kept: there is no
    /// document to save it to and nothing to open one from. The editor is drawn
    /// as an inert page until a project is open, rather than as one that takes
    /// text and quietly loses it.
    pub(crate) fn accepts_writing(&self) -> bool {
        self.project.is_some()
    }

    /// Returns the project-relative path of the document being edited.
    pub(crate) fn open_document_path(&self) -> Option<&Path> {
        self.open_document.as_ref().map(Document::relative_path)
    }

    /// Returns the project directory a new document would be created in.
    ///
    /// A document is created beside the one being edited, so that a new
    /// chapter joins the chapters and a new note joins the references. With
    /// nothing open there is nothing to sit beside, and it goes where the
    /// drafts are kept. The sidebar names the directory before anything is
    /// created, so the choice is never a surprise.
    pub(crate) fn new_document_directory(&self) -> &Path {
        self.open_document_path()
            .and_then(Path::parent)
            .filter(|directory| !directory.as_os_str().is_empty())
            .unwrap_or(Path::new(DEFAULT_DOCUMENT_DIRECTORY))
    }
}

/// The explorer row the pointer is over.
///
/// A drag is read from where the pointer is rather than from the widget that
/// was pressed, because the row's button takes the press for itself: it is what
/// opens a document, and a press it did not see is a click it would not report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HoveredEntry {
    /// A document row, which a drag can carry or be placed against.
    Document {
        /// The document the row draws.
        relative_path: PathBuf,
        /// Where in the row the pointer is, which is what a drop against it
        /// means.
        place: DropPlace,
    },
    /// A directory row, which a drag can be let go over.
    Directory(PathBuf),
}

/// What letting a dragged document go against a document's row asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DropPlace {
    /// The document goes in that row's directory, ahead of the row.
    Before,
    /// The document goes under that row, as one of the chapter's scenes.
    Under,
    /// The document goes in that row's directory, after the row.
    After,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    BeginCreateDocument,
    BeginCreateProject,
    CancelCreateDocument,
    CancelCreateProject,
    ChooseProjectParent,
    CreateDocument,
    Edit(text_editor::Action),
    EntryHovered(Option<HoveredEntry>),
    ModifiersChanged(keyboard::Modifiers),
    NewDocumentNameChanged(String),
    NewProjectNameChanged(String),
    OpenDocument(PathBuf),
    OpenProject,
    OpenProjectFolderPicked(Option<PathBuf>),
    PointerPressed,
    PointerReleased,
    ProjectParentPicked(Option<PathBuf>),
    SaveDocument,
    ToggleProjectDirectory(PathBuf),
    ToggleSidebar,
}

fn boot() -> (Lantern, Task<Message>) {
    let editor_id = Id::unique();
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
            creating_document: false,
            new_document_name: String::new(),
            hovered_entry: None,
            dragged_document: None,
            editor: text_editor::Content::new(),
            editor_id,
            editor_font_size: DEFAULT_EDITOR_FONT_SIZE,
            editor_redraw_epoch: false,
            modifiers: keyboard::Modifiers::default(),
            sidebar_collapsed: false,
            interface_theme,
            theme_error,
        },
        // Lantern opens without a project, so the editor is inert and takes no
        // focus. Opening a document is what puts the caret in it.
        Task::none(),
    )
}

fn update(lantern: &mut Lantern, message: Message) -> Task<Message> {
    match message {
        Message::BeginCreateDocument => {
            lantern.creating_document = true;
            lantern.new_document_name.clear();
            lantern.project_error = None;
        }
        Message::BeginCreateProject => {
            lantern.creating_project = true;
            lantern.new_project_name.clear();
            lantern.project_error = None;
        }
        Message::CancelCreateDocument => {
            lantern.creating_document = false;
            lantern.new_document_name.clear();
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
        Message::CreateDocument => {
            if let Some(relative_path) = create_document(lantern)
                && open_document(lantern, &relative_path)
            {
                return operation::focus(lantern.editor_id.clone());
            }
        }
        Message::Edit(action) => edit_document(lantern, action),
        Message::EntryHovered(entry) => lantern.hovered_entry = entry,
        Message::ModifiersChanged(modifiers) => lantern.modifiers = modifiers,
        Message::NewDocumentNameChanged(name) => lantern.new_document_name = name,
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
        Message::PointerPressed => {
            // Only a document is carried. A directory is a place to put one,
            // and dragging a whole folder would move everything below it.
            if let Some(HoveredEntry::Document { relative_path, .. }) = &lantern.hovered_entry {
                lantern.dragged_document = Some(relative_path.clone());
            }
        }
        Message::PointerReleased => drop_dragged_document(lantern),
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

/// Decodes the window's icon into the pixels Iced hands the system.
///
/// A window with no icon is given the system's default one, so every failure
/// here costs the mark and nothing else. That is worth more than refusing to
/// open over a picture.
fn window_icon() -> Option<iced::window::Icon> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(WINDOW_ICON_BYTES));

    // Whatever the file is stored as - palette, grayscale, no alpha, sixteen
    // bits a channel - these ask the decoder for the eight-bit RGBA that
    // `from_rgba` requires.
    decoder.set_transformations(png::Transformations::normalize_to_color8());

    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut pixels).ok()?;

    pixels.truncate(frame.buffer_size());

    if frame.color_type != png::ColorType::Rgba {
        return None;
    }

    iced::window::icon::from_rgba(pixels, frame.width, frame.height).ok()
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
            lantern.creating_document = false;
            lantern.new_document_name.clear();
            lantern.hovered_entry = None;
            lantern.dragged_document = None;
        }
        Err(error) => lantern.project_error = Some(error.to_string()),
    }
}

/// Creates a document from the typed name and reveals it in the explorer.
///
/// Returns the new document's project-relative path so that the caller can open
/// it, or `None` when nothing was created; a failure is reported in the sidebar
/// and leaves the name in the field to be corrected.
fn create_document(lantern: &mut Lantern) -> Option<PathBuf> {
    // A document created under a chapter is one of its scenes, and the
    // chapter's file is written from those. An open chapter has to be on disk
    // before that happens, or it would be rebuilt around the text on disk and
    // then saved over with the text in the editor, taking the new scene with
    // it. Nothing else in the editor is touched by creating a document.
    if lantern.open_document_path().is_some_and(is_chapter) {
        save_open_document(lantern);
    }

    let directory = lantern.new_document_directory().to_owned();
    let created = {
        let project = lantern.project.as_ref()?;

        lantern.project_service.create_document(
            project,
            &directory,
            lantern.new_document_name.clone(),
        )
    };

    let document = match created {
        Ok(document) => document,
        Err(error) => {
            lantern.project_error = Some(error.to_string());
            return None;
        }
    };

    lantern.creating_document = false;
    lantern.new_document_name.clear();
    lantern.project_error = None;

    reveal_directory(lantern, &directory);
    load_visible_directories(lantern);

    Some(document.relative_path().to_owned())
}

/// Carries out the drop a released drag was asking for.
///
/// A press that never left its own row is a click rather than a drag: the row
/// under the pointer is still the document itself, which asks for nothing, and
/// the row's button emits the message that opens it. Letting go anywhere else -
/// over the editor, over the sidebar's own controls - names no row either, and
/// the drag is simply dropped.
fn drop_dragged_document(lantern: &mut Lantern) {
    let Some(relative_path) = lantern.dragged_document.take() else {
        return;
    };

    // A drop can rewrite the file in the editor: a chapter is written from the
    // scenes under it, and this drop may be giving it one or taking one away.
    // An open chapter's edits reach disk first, so that it is rebuilt around
    // them rather than over them, and [`refresh_open_document`] reads back what
    // the drop made of them. The document being dragged is not saved, because
    // moving a document is not editing it: it keeps its unsaved text and is
    // written where it lands, when the author asks.
    if lantern
        .open_document_path()
        .is_some_and(|open| is_chapter(open) && open != relative_path)
    {
        save_open_document(lantern);
    }

    match lantern.hovered_entry.clone() {
        // Let go over a directory: the document goes into it, and takes
        // whatever place that directory's order leaves it.
        Some(HoveredEntry::Directory(directory)) => {
            // A document let go over the directory it already sits in has not
            // moved, and asking storage to move it there would fail on its own
            // name.
            if relative_path.parent() == Some(directory.as_path()) {
                return;
            }

            let moved = {
                let Some(project) = lantern.project.as_ref() else {
                    return;
                };

                lantern
                    .project_service
                    .move_document(project, &relative_path, &directory)
            };

            finish_drop(lantern, &relative_path, &directory, moved);
        }
        // Let go over the middle of a chapter's row: the document goes under
        // the chapter, as the last of the scenes it is written in.
        Some(HoveredEntry::Document {
            relative_path: hovered_path,
            place: DropPlace::Under,
        }) if is_chapter(&hovered_path) => {
            let Some(directory) = scene_directory(&hovered_path) else {
                return;
            };

            // A scene let go over the chapter it is already under has not moved.
            if hovered_path == relative_path || relative_path.parent() == Some(directory.as_path())
            {
                return;
            }

            let moved = {
                let Some(project) = lantern.project.as_ref() else {
                    return;
                };

                lantern
                    .project_service
                    .move_document(project, &relative_path, &directory)
            };

            finish_drop(lantern, &relative_path, &directory, moved);
        }
        // Let go against another document: the drop names a place in that
        // document's directory, which is recorded as the author's order.
        Some(HoveredEntry::Document {
            relative_path: hovered_path,
            place,
        }) => {
            if hovered_path == relative_path {
                return;
            }

            let Some(directory) = hovered_path.parent().map(Path::to_owned) else {
                return;
            };
            let before = insertion_anchor(
                lantern,
                &directory,
                &hovered_path,
                place == DropPlace::Before,
            );

            let placed = {
                let Some(project) = lantern.project.as_ref() else {
                    return;
                };

                lantern.project_service.place_document(
                    project,
                    &relative_path,
                    &directory,
                    before.as_deref(),
                )
            };

            finish_drop(lantern, &relative_path, &directory, placed);
        }
        None => {}
    }
}

/// Returns the document a dropped one should stand before, if any.
///
/// Let go over a row's top edge, a document stands before that row; let go
/// anywhere else on it, before whichever document follows. `None` asks for the
/// end of the directory, which is also the answer when the listing the row came
/// from is no longer held - the drop is honoured rather than abandoned over
/// bookkeeping the author cannot see.
fn insertion_anchor(
    lantern: &Lantern,
    directory: &Path,
    hovered: &Path,
    above: bool,
) -> Option<String> {
    let documents: Vec<&str> = lantern
        .explorer
        .listing(directory)?
        .iter()
        .filter(|entry| !entry.is_directory())
        .map(|entry| entry.name())
        .collect();

    let hovered_name = hovered.file_name().and_then(|name| name.to_str())?;
    let index = documents.iter().position(|name| *name == hovered_name)?;

    if above {
        return Some(documents[index].to_owned());
    }

    documents.get(index + 1).map(|name| (*name).to_owned())
}

/// Puts the explorer and the editor back in step with a drop that has happened.
fn finish_drop(
    lantern: &mut Lantern,
    relative_path: &Path,
    directory: &Path,
    result: Result<PathBuf, lantern_service::ProjectServiceError>,
) {
    let moved_path = match result {
        Ok(moved_path) => moved_path,
        Err(error) => {
            lantern.project_error = Some(error.to_string());
            return;
        }
    };

    // The document in the editor is the same document; only its path may have
    // changed. Told about the move, it keeps its text, its unsaved edits and
    // the caret where the author left it, and the next save writes where the
    // file is now.
    if lantern.open_document_path() == Some(relative_path)
        && let Some(document) = lantern.open_document.as_mut()
    {
        document.record_moved(moved_path);
    }

    lantern.project_error = None;

    // Both ends of the drop are stale: the directory the document left still
    // lists it, and the one it arrived in either does not hold it or holds it
    // in the sequence it had before.
    if let Some(parent) = relative_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        lantern.explorer.forget_listing(parent);
    }

    reveal_directory(lantern, directory);
    load_visible_directories(lantern);
    refresh_open_document(lantern);
}

/// Reads the open document again when a drop has rewritten its file.
///
/// A chapter's file is written from the scenes under it, so a drop that gives it
/// one or takes one away leaves what is in the editor behind. Only a chapter can
/// be rewritten this way, and only text that has actually changed is replaced,
/// so every other drop leaves the editor and the caret exactly as they were.
fn refresh_open_document(lantern: &mut Lantern) {
    let Some(relative_path) = lantern.open_document_path().map(Path::to_owned) else {
        return;
    };

    // Text an author has not saved is theirs, and is never replaced by what is
    // on disk - a chapter carrying edits is left alone until they are written.
    if !is_chapter(&relative_path) || lantern.unsaved_edits {
        return;
    }

    let Some(project) = lantern.project.as_ref() else {
        return;
    };

    // A chapter that cannot be read again is left in the editor as it stands;
    // the author's text is worth more than agreement with the disk.
    let Ok(document) = lantern
        .project_service
        .open_document(project, &relative_path)
    else {
        return;
    };

    if !document.differs_from(&lantern.editor.text()) {
        return;
    }

    lantern.editor = text_editor::Content::with_text(document.content());
    lantern.open_document = Some(document);
    lantern.unsaved_edits = false;
}

/// Expands the way down to a directory and has its listing read again.
///
/// A directory Lantern has just written into is stale in memory, and may not
/// even be drawn. Expanding every directory above it and forgetting its listing
/// leaves both to [`load_visible_directories`], which the caller runs next.
fn reveal_directory(lantern: &mut Lantern, directory: &Path) {
    for ancestor in directory
        .ancestors()
        .filter(|ancestor| !ancestor.as_os_str().is_empty())
    {
        lantern.explorer.expand(ancestor.to_owned());
    }

    lantern.explorer.forget_listing(directory);

    // The directory above it too: a chapter that has just been given its first
    // scene is a row that was not expandable when its own directory was listed.
    // Never the root, whose listing is the one everything else is found
    // through, and which holds the same workspace directories either way.
    if let Some(parent) = directory
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        lantern.explorer.forget_listing(parent);
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
        // A row's button takes the press and the release for itself, so a drag
        // is read from the window rather than from the widget. Letting go
        // outside the explorer has to arrive too, or a drag abandoned over the
        // editor would still be held when the pointer returned.
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
            Some(Message::PointerPressed)
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(Message::PointerReleased)
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
