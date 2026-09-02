//! Application use-cases and business rules for Lantern.

use lantern_core::{
    DocumentName, DocumentNameError, ProjectName, ProjectNameError, has_editable_extension,
    order_documents,
};
use lantern_store::{ProjectStore, StoreError, ThemeStore};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use lantern_core::{
    Color, DEFAULT_DOCUMENT_DIRECTORY, Document, DocumentEncoding, LineEnding, Project,
    ProjectEntry, Theme, ThemeMode, ThemePalette, WORKSPACE_DIRECTORIES,
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
            return Ok(order_documents(
                entries,
                &self.store.document_order(project, relative_path),
            ));
        }

        // The root's own order is fixed by the workspace it presents, so the
        // author's ordering does not reach it.

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

    /// Creates an empty document inside one of a project's directories.
    ///
    /// The typed name is validated and given an editable extension, so that a
    /// document Lantern creates is one it can open again. A name already taken
    /// is a failure rather than a document emptied.
    pub fn create_document(
        &self,
        project: &Project,
        directory: &Path,
        name: impl Into<String>,
    ) -> Result<Document, ProjectServiceError> {
        let name = DocumentName::new(name)?;
        let relative_path = directory.join(name.as_str());

        self.store.create_document(project, &relative_path)?;

        Ok(self.store.read_document(project, &relative_path)?)
    }

    /// Moves a document into another of the project's directories.
    ///
    /// The document keeps its name; only the directory holding it changes.
    /// Returns the project-relative path it now has, which the caller needs in
    /// order to go on describing the document it already holds open.
    pub fn move_document(
        &self,
        project: &Project,
        relative_path: &Path,
        directory: &Path,
    ) -> Result<PathBuf, ProjectServiceError> {
        Ok(self
            .store
            .move_document(project, relative_path, directory)?)
    }

    /// Puts a document in a directory, in a place the author has chosen.
    ///
    /// The document is moved when it comes from another directory, and either
    /// way the directory's order is recorded with the document standing before
    /// `before` - or last, when no document is named. The whole resulting
    /// sequence is written, so a directory that had no order acquires the one
    /// it was being drawn in, with the placed document moved within it.
    ///
    /// Returns the project-relative path the document now has.
    pub fn place_document(
        &self,
        project: &Project,
        relative_path: &Path,
        directory: &Path,
        before: Option<&str>,
    ) -> Result<PathBuf, ProjectServiceError> {
        let moved_path = if relative_path.parent() == Some(directory) {
            relative_path.to_owned()
        } else {
            self.move_document(project, relative_path, directory)?
        };

        let Some(name) = moved_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            // A name that is not UTF-8 cannot be recorded. The document has
            // still moved; it simply keeps the place its listing gives it.
            return Ok(moved_path);
        };

        // Ordering a document against itself asks for the sequence that is
        // already there.
        if before == Some(name.as_str()) {
            return Ok(moved_path);
        }

        let mut order: Vec<String> = self
            .list_directory(project, directory)?
            .iter()
            .filter(|entry| !entry.is_directory())
            .map(|entry| entry.name().to_owned())
            .collect();

        order.retain(|listed| listed != &name);

        match before.and_then(|before| order.iter().position(|listed| listed == before)) {
            Some(index) => order.insert(index, name),
            None => order.push(name),
        }

        self.store.set_document_order(project, directory, &order)?;

        Ok(moved_path)
    }

    /// Opens a supported Markdown or plain-text document.
    pub fn open_document(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Document, ProjectServiceError> {
        if !has_editable_extension(relative_path) {
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

        if !has_editable_extension(relative_path) {
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
    /// The requested document name violated a domain invariant.
    #[error(transparent)]
    InvalidDocumentName(#[from] DocumentNameError),
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
