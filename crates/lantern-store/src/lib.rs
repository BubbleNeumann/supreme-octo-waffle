//! Persistence interfaces and implementations for Lantern.

mod order;
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

    /// Creates one directory inside a project, accepting one already there.
    ///
    /// Callers use this to put a directory a project needs in place, so an
    /// existing directory is success rather than a conflict. Anything else
    /// standing in its place is a failure.
    fn create_directory(&self, project: &Project, relative_path: &Path) -> Result<(), StoreError>;

    /// Creates one empty document inside a project.
    ///
    /// The document must not already be there: creating never writes over a
    /// file, because an author who names a document that exists means a new
    /// document rather than an empty one where their work used to be.
    fn create_document(&self, project: &Project, relative_path: &Path) -> Result<(), StoreError>;

    /// Moves one document into another directory inside the same project.
    ///
    /// The document keeps its own name, so only the directory holding it
    /// changes. Returns the project-relative path the document now has.
    fn move_document(
        &self,
        project: &Project,
        relative_path: &Path,
        directory: &Path,
    ) -> Result<PathBuf, StoreError>;

    /// Deletes one document inside a project.
    ///
    /// The path must name an ordinary file. Directories are the author's to
    /// arrange and are never removed on their behalf.
    fn delete_document(&self, project: &Project, relative_path: &Path) -> Result<(), StoreError>;

    /// Reads the order an author has given one directory's documents.
    ///
    /// A directory that has never been ordered, and one whose recorded order
    /// cannot be read, both give an empty order. Lantern's own state is never
    /// required to list a project.
    fn document_order(&self, project: &Project, directory: &Path) -> Vec<String>;

    /// Records the order of one directory's documents.
    ///
    /// The names are stored as given, so a caller passing the directory's whole
    /// listing records a complete order and one passing an empty slice returns
    /// the directory to the order storage lists it in.
    fn set_document_order(
        &self,
        project: &Project,
        directory: &Path,
        names: &[String],
    ) -> Result<(), StoreError>;

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

    fn create_directory(&self, project: &Project, relative_path: &Path) -> Result<(), StoreError> {
        if relative_path.as_os_str().is_empty()
            || !is_safe_relative_path(relative_path)
            || is_lantern_internal_path(relative_path)
        {
            return Err(StoreError::UnsafeProjectPath(relative_path.to_owned()));
        }

        let directory = project.root().join(relative_path);

        match fs::create_dir_all(&directory) {
            Ok(()) => {}
            // `create_dir_all` reports an existing file as another kind of
            // failure on some systems; the check below names it properly.
            Err(_) if directory.exists() => {}
            Err(source) => {
                return Err(StoreError::Io {
                    path: directory,
                    source,
                });
            }
        }

        // Resolving afterwards rejects anything that leaves the project, such
        // as a symbolic link standing where the directory belongs.
        let resolved = resolve_project_path(project, relative_path)?;

        if !resolved.is_dir() {
            return Err(StoreError::NotDirectory(resolved));
        }

        Ok(())
    }

    fn create_document(&self, project: &Project, relative_path: &Path) -> Result<(), StoreError> {
        if !is_safe_relative_path(relative_path) || is_lantern_internal_path(relative_path) {
            return Err(StoreError::UnsafeProjectPath(relative_path.to_owned()));
        }

        let Some(file_name) = relative_path.file_name() else {
            return Err(StoreError::UnsafeProjectPath(relative_path.to_owned()));
        };

        // The document does not exist yet, so it is the directory holding it
        // that is resolved and checked to be inside the project.
        let directory =
            resolve_project_path(project, relative_path.parent().unwrap_or(Path::new("")))?;

        if !directory.is_dir() {
            return Err(StoreError::NotDirectory(directory));
        }

        let document_path = directory.join(file_name);

        match fs::File::create_new(&document_path) {
            Ok(_) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                Err(StoreError::AlreadyExists(document_path))
            }
            Err(source) => Err(StoreError::Io {
                path: document_path,
                source,
            }),
        }
    }

    fn move_document(
        &self,
        project: &Project,
        relative_path: &Path,
        directory: &Path,
    ) -> Result<PathBuf, StoreError> {
        let Some(file_name) = relative_path.file_name() else {
            return Err(StoreError::UnsafeProjectPath(relative_path.to_owned()));
        };

        let document_path = resolve_project_path(project, relative_path)?;

        if !document_path.is_file() {
            return Err(StoreError::NotFile(document_path));
        }

        let destination = resolve_project_path(project, directory)?;

        if !destination.is_dir() {
            return Err(StoreError::NotDirectory(destination));
        }

        let moved_path = destination.join(file_name);

        // `fs::rename` replaces a file already standing at the destination on
        // Unix while refusing on Windows, so the document in the way is named
        // here rather than left to the platform. A file that appears between
        // this check and the rename is a race Lantern holds no lock against.
        if moved_path.exists() {
            return Err(StoreError::AlreadyExists(moved_path));
        }

        fs::rename(&document_path, &moved_path).map_err(|source| StoreError::Io {
            path: moved_path,
            source,
        })?;

        // Built from the caller's own path rather than from the resolved one,
        // so that the document keeps the exact bytes the system reported.
        Ok(directory.join(file_name))
    }

    fn delete_document(&self, project: &Project, relative_path: &Path) -> Result<(), StoreError> {
        let document_path = resolve_project_path(project, relative_path)?;

        if !document_path.is_file() {
            return Err(StoreError::NotFile(document_path));
        }

        fs::remove_file(&document_path).map_err(|source| StoreError::Io {
            path: document_path,
            source,
        })
    }

    fn document_order(&self, project: &Project, directory: &Path) -> Vec<String> {
        order::read(project, directory)
    }

    fn set_document_order(
        &self,
        project: &Project,
        directory: &Path,
        names: &[String],
    ) -> Result<(), StoreError> {
        order::write(project, directory, names)
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
    /// The document order could not be written as a TOML file.
    ///
    /// The order is built from names storage itself reported, so this is a bug
    /// rather than something an author can bring about.
    #[error("the document order could not be written")]
    OrderNotWritable,
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
