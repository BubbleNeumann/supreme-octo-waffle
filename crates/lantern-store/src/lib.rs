//! Persistence interfaces and implementations for Lantern.

use lantern_core::{Document, Project, ProjectEntry, ProjectEntryKind, ProjectName};
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// Persistence operations needed to open and create projects.
pub trait ProjectStore {
    /// Opens an existing directory as a project.
    fn open_project(&self, root: &Path) -> Result<Project, StoreError>;

    /// Creates a project as one new child directory of `parent`.
    fn create_project(&self, parent: &Path, name: &ProjectName) -> Result<Project, StoreError>;

    /// Lists one directory inside a project without recursively traversing it.
    fn list_directory(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Vec<ProjectEntry>, StoreError>;

    /// Reads one UTF-8 document using a project-relative path.
    fn read_document(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Document, StoreError>;
}

/// Filesystem-backed project persistence.
#[derive(Debug, Default)]
pub struct FsProjectStore;

impl ProjectStore for FsProjectStore {
    fn open_project(&self, root: &Path) -> Result<Project, StoreError> {
        let metadata = fs::metadata(root).map_err(|source| StoreError::Io {
            path: root.to_owned(),
            source,
        })?;

        if !metadata.is_dir() {
            return Err(StoreError::NotDirectory(root.to_owned()));
        }

        let root = root.canonicalize().map_err(|source| StoreError::Io {
            path: root.to_owned(),
            source,
        })?;

        Ok(Project::from_verified_root(root))
    }

    fn create_project(&self, parent: &Path, name: &ProjectName) -> Result<Project, StoreError> {
        let parent = self.open_project(parent)?.root().to_owned();
        let root = parent.join(name.as_str());

        fs::create_dir(&root).map_err(|source| StoreError::Io {
            path: root.clone(),
            source,
        })?;

        self.open_project(&root)
    }

    fn list_directory(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Vec<ProjectEntry>, StoreError> {
        let directory = resolve_project_path(project, relative_path)?;

        if !directory.is_dir() {
            return Err(StoreError::NotDirectory(directory));
        }

        let mut entries = Vec::new();
        let directory_entries = fs::read_dir(&directory).map_err(|source| StoreError::Io {
            path: directory.clone(),
            source,
        })?;

        for entry in directory_entries {
            let entry = entry.map_err(|source| StoreError::Io {
                path: directory.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();

            if relative_path.as_os_str().is_empty() && name.eq_ignore_ascii_case(".lantern") {
                continue;
            }

            let file_type = entry.file_type().map_err(|source| StoreError::Io {
                path: entry.path(),
                source,
            })?;
            let kind = if file_type.is_dir() {
                ProjectEntryKind::Directory
            } else {
                ProjectEntryKind::File
            };

            entries.push(ProjectEntry::from_verified_path(
                relative_path.join(&name),
                name,
                kind,
            ));
        }

        entries.sort_by(
            |left, right| match (left.is_directory(), right.is_directory()) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => left.name().to_lowercase().cmp(&right.name().to_lowercase()),
            },
        );

        Ok(entries)
    }

    fn read_document(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Document, StoreError> {
        let document_path = resolve_project_path(project, relative_path)?;

        if !document_path.is_file() {
            return Err(StoreError::NotFile(document_path));
        }

        let content = fs::read_to_string(&document_path).map_err(|source| StoreError::Io {
            path: document_path,
            source,
        })?;

        Ok(Document::from_verified_content(
            relative_path.to_owned(),
            content,
        ))
    }
}

fn resolve_project_path(project: &Project, relative_path: &Path) -> Result<PathBuf, StoreError> {
    if !is_safe_relative_path(relative_path) || is_lantern_internal_path(relative_path) {
        return Err(StoreError::UnsafeProjectPath(relative_path.to_owned()));
    }

    let unresolved = project.root().join(relative_path);
    let resolved = unresolved.canonicalize().map_err(|source| StoreError::Io {
        path: unresolved,
        source,
    })?;

    if !resolved.starts_with(project.root()) {
        return Err(StoreError::UnsafeProjectPath(relative_path.to_owned()));
    }

    Ok(resolved)
}

fn is_safe_relative_path(path: &Path) -> bool {
    path.as_os_str().is_empty()
        || path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_lantern_internal_path(path: &Path) -> bool {
    path.components().next().is_some_and(|component| {
        matches!(component, Component::Normal(name) if name.to_string_lossy().eq_ignore_ascii_case(".lantern"))
    })
}

/// A filesystem persistence failure.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The selected path exists but is not a directory.
    #[error("'{0}' is not a directory")]
    NotDirectory(PathBuf),
    /// The selected path exists but is not an ordinary file.
    #[error("'{}' is not a file", .0.display())]
    NotFile(PathBuf),
    /// A caller attempted to access outside the opened project.
    #[error("'{}' is not a safe project-relative path", .0.display())]
    UnsafeProjectPath(PathBuf),
    /// The operating system rejected a filesystem operation.
    #[error("could not access '{}': {source}", path.display())]
    Io {
        /// Path involved in the failed operation.
        path: PathBuf,
        /// Underlying operating-system error.
        #[source]
        source: io::Error,
    },
}
