use iced::widget::{Id, operation, text_editor};
use iced::{Event, Size, Subscription, Task, event, keyboard};
use lantern_service::{FsProjectService, Project, ProjectEntry};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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
    project_service: FsProjectService,
    pub(crate) project: Option<Project>,
    directory_entries: HashMap<PathBuf, Vec<ProjectEntry>>,
    expanded_directories: HashSet<PathBuf>,
    pub(crate) project_tree: Vec<ProjectTreeRow>,
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
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectTreeRow {
    pub(crate) entry: ProjectEntry,
    pub(crate) depth: usize,
    pub(crate) expanded: bool,
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

    (
        Lantern {
            project_service: FsProjectService::filesystem(),
            project: None,
            directory_entries: HashMap::new(),
            expanded_directories: HashSet::new(),
            project_tree: Vec::new(),
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
        Message::Edit(action) => {
            if let text_editor::Action::Scroll { lines } = action {
                if lines == 0 {
                    return Task::none();
                }

                if lantern.modifiers.control() {
                    let zoom = -FONT_ZOOM_STEP * (lines as f32).signum();
                    lantern.editor_font_size = (lantern.editor_font_size + zoom)
                        .clamp(MIN_EDITOR_FONT_SIZE, MAX_EDITOR_FONT_SIZE);
                } else {
                    lantern
                        .editor
                        .perform(text_editor::Action::Scroll { lines });
                }

                // The tiny-skia compositor uses incremental damage tracking. An
                // invisible, changing pane primitive makes the editor viewport's
                // damage explicit and prevents stale pixels at its bottom edge.
                lantern.editor_redraw_epoch = !lantern.editor_redraw_epoch;
            } else {
                lantern.editor.perform(action);
            }
        }
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
            lantern.directory_entries.clear();
            lantern.expanded_directories.clear();
            lantern.project_tree.clear();
            lantern.open_document = None;
            lantern.editor = text_editor::Content::new();

            match root_entries {
                Ok(entries) => {
                    lantern.directory_entries.insert(PathBuf::new(), entries);
                    lantern.project_error = None;
                    rebuild_project_tree(lantern);
                }
                Err(error) => lantern.project_error = Some(error.to_string()),
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
    if lantern.expanded_directories.remove(&relative_path) {
        rebuild_project_tree(lantern);
        return;
    }

    if !lantern.directory_entries.contains_key(&relative_path) {
        let Some(project) = lantern.project.as_ref() else {
            return;
        };

        match lantern
            .project_service
            .list_directory(project, &relative_path)
        {
            Ok(entries) => {
                lantern
                    .directory_entries
                    .insert(relative_path.clone(), entries);
                lantern.project_error = None;
            }
            Err(error) => {
                lantern.project_error = Some(error.to_string());
                return;
            }
        }
    }

    lantern.expanded_directories.insert(relative_path);
    rebuild_project_tree(lantern);
}

fn rebuild_project_tree(lantern: &mut Lantern) {
    let mut project_tree = Vec::new();
    append_visible_entries(
        &lantern.directory_entries,
        &lantern.expanded_directories,
        Path::new(""),
        0,
        &mut project_tree,
    );
    lantern.project_tree = project_tree;
}

fn append_visible_entries(
    directory_entries: &HashMap<PathBuf, Vec<ProjectEntry>>,
    expanded_directories: &HashSet<PathBuf>,
    directory: &Path,
    depth: usize,
    project_tree: &mut Vec<ProjectTreeRow>,
) {
    let Some(entries) = directory_entries.get(directory) else {
        return;
    };

    for entry in entries {
        let expanded = entry.is_directory() && expanded_directories.contains(entry.relative_path());

        project_tree.push(ProjectTreeRow {
            entry: entry.clone(),
            depth,
            expanded,
        });

        if expanded {
            append_visible_entries(
                directory_entries,
                expanded_directories,
                entry.relative_path(),
                depth + 1,
                project_tree,
            );
        }
    }
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
