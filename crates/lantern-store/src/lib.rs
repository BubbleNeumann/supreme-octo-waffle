//! Persistence interfaces and implementations for Lantern.

mod theme;

pub use theme::{FsThemeStore, ThemeStore};

use lantern_core::{Document, Project, ProjectEntry, ProjectEntryKind, ProjectName, ThemeError};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// The directory reserved for Lantern-owned state inside a project.
const LANTERN_DIRECTORY: &str = ".lantern";

/// The largest document Lantern will load into the editor.
///
/// The editor holds documents in memory as a single string, so an accidental
/// click on a multi-gigabyte log must fail fast instead of exhausting memory.
pub const MAX_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;

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

    /// Overwrites one existing document with already-encoded text.
    ///
    /// Callers pass the exact bytes to store; restoring a document's original
    /// line endings and byte order mark is the service layer's responsibility.
    fn save_document(
        &self,
        project: &Project,
        relative_path: &Path,
        raw_content: &str,
    ) -> Result<(), StoreError>;
}

/// Filesystem-backed project persistence.
#[derive(Debug, Default)]
pub struct FsProjectStore;

impl ProjectStore for FsProjectStore {
    fn open_project(&self, root: &Path) -> Result<Project, StoreError> {
        Ok(Project::from_verified_root(verify_directory(root)?))
    }

    fn create_project(&self, parent: &Path, name: &ProjectName) -> Result<Project, StoreError> {
        let parent = verify_directory(parent)?;
        let root = parent.join(name.as_str());

        match fs::create_dir(&root) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                return Err(StoreError::AlreadyExists(root));
            }
            Err(source) => return Err(StoreError::Io { path: root, source }),
        }

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
            let file_name = entry.file_name();

            if relative_path.as_os_str().is_empty()
                && file_name
                    .as_encoded_bytes()
                    .eq_ignore_ascii_case(LANTERN_DIRECTORY.as_bytes())
            {
                continue;
            }

            // `DirEntry::file_type` reports the link itself rather than its
            // target, which would classify a symlinked directory as a file and
            // leave it impossible to expand. A broken link stays a file.
            let kind = if fs::metadata(entry.path()).is_ok_and(|metadata| metadata.is_dir()) {
                ProjectEntryKind::Directory
            } else {
                ProjectEntryKind::File
            };

            // The path keeps the operating system's exact bytes so that it still
            // resolves; only the display name is lossily converted.
            entries.push(ProjectEntry::from_verified_path(
                relative_path.join(&file_name),
                file_name.to_string_lossy().into_owned(),
                kind,
            ));
        }

        // Directories first, then case-insensitive by name. The key is cached so
        // that lowercasing happens once per entry rather than once per compare.
        entries.sort_by_cached_key(|entry| (!entry.is_directory(), entry.name().to_lowercase()));

        Ok(entries)
    }

    fn read_document(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Document, StoreError> {
        let document_path = resolve_project_path(project, relative_path)?;
        let metadata = fs::metadata(&document_path).map_err(|source| StoreError::Io {
            path: document_path.clone(),
            source,
        })?;

        if !metadata.is_file() {
            return Err(StoreError::NotFile(document_path));
        }

        if metadata.len() > MAX_DOCUMENT_BYTES {
            return Err(StoreError::DocumentTooLarge {
                path: document_path,
                bytes: metadata.len(),
                limit: MAX_DOCUMENT_BYTES,
            });
        }

        let bytes = fs::read(&document_path).map_err(|source| StoreError::Io {
            path: document_path.clone(),
            source,
        })?;
        let raw_content =
            String::from_utf8(bytes).map_err(|_| StoreError::NotUtf8(document_path))?;

        Ok(Document::from_verified_content(
            relative_path.to_owned(),
            raw_content,
        ))
    }

    fn save_document(
        &self,
        project: &Project,
        relative_path: &Path,
        raw_content: &str,
    ) -> Result<(), StoreError> {
        let document_path = resolve_project_path(project, relative_path)?;
        let metadata = fs::metadata(&document_path).map_err(|source| StoreError::Io {
            path: document_path.clone(),
            source,
        })?;

        if !metadata.is_file() {
            return Err(StoreError::NotFile(document_path));
        }

        let directory = document_path.parent().unwrap_or_else(|| project.root());

        // Write a sibling temporary file and rename it over the target, so that
        // a crash or a full disk mid-save cannot leave a truncated document
        // where the author's work used to be.
        let mut temporary =
            tempfile::NamedTempFile::new_in(directory).map_err(|source| StoreError::Io {
                path: directory.to_owned(),
                source,
            })?;

        temporary
            .write_all(raw_content.as_bytes())
            .map_err(|source| StoreError::Io {
                path: document_path.clone(),
                source,
            })?;

        temporary
            .as_file()
            .sync_all()
            .map_err(|source| StoreError::Io {
                path: document_path.clone(),
                source,
            })?;

        // `NamedTempFile` creates private files; keep the document's own mode.
        let _ = temporary.as_file().set_permissions(metadata.permissions());

        temporary
            .persist(&document_path)
            .map_err(|error| StoreError::Io {
                path: document_path,
                source: error.error,
            })?;

        Ok(())
    }
}

fn verify_directory(path: &Path) -> Result<PathBuf, StoreError> {
    let metadata = fs::metadata(path).map_err(|source| StoreError::Io {
        path: path.to_owned(),
        source,
    })?;

    if !metadata.is_dir() {
        return Err(StoreError::NotDirectory(path.to_owned()));
    }

    path.canonicalize().map_err(|source| StoreError::Io {
        path: path.to_owned(),
        source,
    })
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
        matches!(component, Component::Normal(name)
            if name.as_encoded_bytes().eq_ignore_ascii_case(LANTERN_DIRECTORY.as_bytes()))
    })
}

/// A filesystem persistence failure.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The selected path exists but is not a directory.
    #[error("'{}' is not a directory", .0.display())]
    NotDirectory(PathBuf),
    /// The selected path exists but is not an ordinary file.
    #[error("'{}' is not a file", .0.display())]
    NotFile(PathBuf),
    /// A directory with the requested project name is already present.
    #[error("'{}' already exists", .0.display())]
    AlreadyExists(PathBuf),
    /// The document holds bytes that are not valid UTF-8 text.
    #[error("'{}' is not a UTF-8 text document", .0.display())]
    NotUtf8(PathBuf),
    /// The document is larger than Lantern will open in the editor.
    #[error("'{}' is {bytes} bytes, above the {limit} byte editing limit", path.display())]
    DocumentTooLarge {
        /// Path of the document that was too large.
        path: PathBuf,
        /// Actual size of the document in bytes.
        bytes: u64,
        /// Largest size Lantern will open.
        limit: u64,
    },
    /// A caller attempted to access outside the opened project.
    #[error("'{}' is not a safe project-relative path", .0.display())]
    UnsafeProjectPath(PathBuf),
    /// A theme file could not be parsed as TOML.
    #[error("'{}' is not valid TOML: {source}", path.display())]
    ThemeSyntax {
        /// Path of the theme file that could not be parsed.
        path: PathBuf,
        /// Underlying parse failure.
        #[source]
        source: toml::de::Error,
    },
    /// A theme file did not define an entry Lantern needs.
    #[error("'{}' has no {key} entry", path.display())]
    ThemeKeyMissing {
        /// Path of the theme file that was incomplete.
        path: PathBuf,
        /// The table and key that were looked for.
        key: String,
    },
    /// A theme file entry did not describe a usable colour.
    #[error("'{}' entry {key}: {source}", path.display())]
    ThemeColor {
        /// Path of the theme file holding the entry.
        path: PathBuf,
        /// The table and key of the offending entry.
        key: String,
        /// Why the entry could not be resolved.
        #[source]
        source: ThemeError,
    },
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
