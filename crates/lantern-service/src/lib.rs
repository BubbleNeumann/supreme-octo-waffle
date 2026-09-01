//! Application use-cases and business rules for Lantern.

use lantern_core::{ProjectName, ProjectNameError};
use lantern_store::{ProjectStore, StoreError, ThemeStore};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use lantern_core::{
    Color, Document, DocumentEncoding, LineEnding, Project, ProjectEntry, Theme, ThemeMode,
    ThemePalette, WORKSPACE_DIRECTORIES,
};
pub use lantern_store::{FsProjectStore, FsThemeStore};

/// Project-related application use-cases over an injected persistence adapter.
#[derive(Debug)]
pub struct ProjectService<S> {
    store: S,
}

impl<S> ProjectService<S> {
    /// Creates a service using the provided persistence adapter.
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S: ProjectStore> ProjectService<S> {
    /// Opens an existing ordinary directory as a Lantern project.
    ///
    /// The directories in [`WORKSPACE_DIRECTORIES`] are put in place as part of
    /// opening, so a project always presents the same root.
    pub fn open_project(&self, root: &Path) -> Result<Project, ProjectServiceError> {
        let project = self.store.open_project(root)?;

        self.create_workspace(&project)?;

        Ok(project)
    }

    /// Creates and opens a new project directory under an existing parent.
    pub fn create_project(
        &self,
        parent: &Path,
        name: impl Into<String>,
    ) -> Result<Project, ProjectServiceError> {
        let name = ProjectName::new(name)?;
        let project = self.store.create_project(parent, &name)?;

        self.create_workspace(&project)?;

        Ok(project)
    }

    /// Lists one project directory for the explorer.
    ///
    /// The root lists as the workspace directories alone, in the order
    /// [`WORKSPACE_DIRECTORIES`] declares. Everything else the author keeps
    /// beside them is left where it is and not shown. Directories below the
    /// root list in full.
    pub fn list_directory(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Vec<ProjectEntry>, ProjectServiceError> {
        let entries = self.store.list_directory(project, relative_path)?;

        if !relative_path.as_os_str().is_empty() {
            return Ok(entries);
        }

        Ok(WORKSPACE_DIRECTORIES
            .iter()
            .filter_map(|name| {
                entries
                    .iter()
                    .find(|entry| entry.is_directory_named(name))
                    .cloned()
            })
            .collect())
    }

    /// Puts the directories every project keeps in place, leaving others alone.
    fn create_workspace(&self, project: &Project) -> Result<(), ProjectServiceError> {
        for name in WORKSPACE_DIRECTORIES {
            self.store.create_directory(project, Path::new(name))?;
        }

        Ok(())
    }

    /// Opens a supported Markdown or plain-text document.
    pub fn open_document(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Document, ProjectServiceError> {
        if !is_editable_document(relative_path) {
            return Err(ProjectServiceError::UnsupportedDocument(
                relative_path.to_owned(),
            ));
        }

        Ok(self.store.read_document(project, relative_path)?)
    }

    /// Saves edited text back over an open document.
    ///
    /// The document's original line endings and byte order mark are restored, so
    /// that saving an unmodified buffer reproduces the file byte for byte. A
    /// document that saves successfully adopts `content` as its own text, so
    /// that it keeps describing the file that is now on disk.
    pub fn save_document(
        &self,
        project: &Project,
        document: &mut Document,
        content: &str,
    ) -> Result<(), ProjectServiceError> {
        let relative_path = document.relative_path();

        if !is_editable_document(relative_path) {
            return Err(ProjectServiceError::UnsupportedDocument(
                relative_path.to_owned(),
            ));
        }

        self.store
            .save_document(project, relative_path, &document.encoding().apply(content))?;
        document.record_saved(content.to_owned());

        Ok(())
    }
}

fn is_editable_document(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "txt"
            )
        })
}

impl ProjectService<FsProjectStore> {
    /// Creates the desktop service backed by the local filesystem.
    pub fn filesystem() -> Self {
        Self::new(FsProjectStore)
    }
}

/// The desktop project's filesystem-backed service type.
pub type FsProjectService = ProjectService<FsProjectStore>;

/// A failure while opening or creating a project.
#[derive(Debug, Error)]
pub enum ProjectServiceError {
    /// The requested project name violated a domain invariant.
    #[error(transparent)]
    InvalidName(#[from] ProjectNameError),
    /// The selected file is not an editable MVP document format.
    #[error("'{}' is not an editable Markdown or text document", .0.display())]
    UnsupportedDocument(PathBuf),
    /// The persistence adapter could not complete the operation.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Interface theme use-cases over an injected theme adapter.
#[derive(Debug)]
pub struct ThemeService<S> {
    store: S,
}

impl<S> ThemeService<S> {
    /// Creates a service using the provided theme adapter.
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S: ThemeStore> ThemeService<S> {
    /// Lists the themes the interface can offer, ordered by name.
    pub fn available_themes(&self) -> Result<Vec<Theme>, ThemeServiceError> {
        Ok(self.store.list_themes()?)
    }

    /// Loads the theme with the given name.
    pub fn theme(&self, name: &str) -> Result<Theme, ThemeServiceError> {
        self.available_themes()?
            .into_iter()
            .find(|theme| theme.name() == name)
            .ok_or_else(|| ThemeServiceError::UnknownTheme(name.to_owned()))
    }
}

impl ThemeService<FsThemeStore> {
    /// Creates the desktop service backed by theme files on disk.
    pub fn filesystem(search_paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self::new(FsThemeStore::new(search_paths))
    }
}

/// The desktop client's file-backed theme service type.
pub type FsThemeService = ThemeService<FsThemeStore>;

/// A failure while loading an interface theme.
#[derive(Debug, Error)]
pub enum ThemeServiceError {
    /// No installed theme carries the requested name.
    #[error("no theme named '{0}' is installed")]
    UnknownTheme(String),
    /// The theme adapter could not read the installed themes.
    #[error(transparent)]
    Store(#[from] StoreError),
}
