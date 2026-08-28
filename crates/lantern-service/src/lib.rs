//! Application use-cases and business rules for Lantern.

use lantern_core::{ProjectName, ProjectNameError};
use lantern_store::{ProjectStore, StoreError};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use lantern_core::{Document, Project, ProjectEntry};
pub use lantern_store::FsProjectStore;

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
    pub fn open_project(&self, root: &Path) -> Result<Project, ProjectServiceError> {
        Ok(self.store.open_project(root)?)
    }

    /// Creates and opens a new project directory under an existing parent.
    pub fn create_project(
        &self,
        parent: &Path,
        name: impl Into<String>,
    ) -> Result<Project, ProjectServiceError> {
        let name = ProjectName::new(name)?;

        Ok(self.store.create_project(parent, &name)?)
    }

    /// Lists one project directory for the explorer.
    pub fn list_directory(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Vec<ProjectEntry>, ProjectServiceError> {
        Ok(self.store.list_directory(project, relative_path)?)
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
