//! Domain types and invariants shared across Lantern.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// An opened Lantern project rooted at an ordinary directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    root: PathBuf,
}

impl Project {
    /// Creates a project from a canonical directory path verified by storage.
    pub fn from_verified_root(root: PathBuf) -> Self {
        Self { root }
    }

    /// Returns the absolute project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns a user-facing project name derived from the root directory.
    pub fn display_name(&self) -> String {
        self.root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string())
    }
}

/// An opened human-authored text document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    relative_path: PathBuf,
    content: String,
}

impl Document {
    /// Creates a document from content and a project-relative path verified by storage.
    pub fn from_verified_content(relative_path: PathBuf, content: String) -> Self {
        Self {
            relative_path,
            content,
        }
    }

    /// Returns the document's path relative to its project root.
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Returns the document text.
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// One file or directory displayed in a project's explorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEntry {
    relative_path: PathBuf,
    name: String,
    kind: ProjectEntryKind,
}

impl ProjectEntry {
    /// Creates an entry from a project-relative path verified by storage.
    pub fn from_verified_path(
        relative_path: PathBuf,
        name: String,
        kind: ProjectEntryKind,
    ) -> Self {
        Self {
            relative_path,
            name,
            kind,
        }
    }

    /// Returns the entry's path relative to the project root.
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Returns the entry's file name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this entry is an ordinary directory.
    pub fn is_directory(&self) -> bool {
        self.kind == ProjectEntryKind::Directory
    }
}

/// The filesystem kind relevant to the project explorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectEntryKind {
    /// An ordinary directory that can be expanded.
    Directory,
    /// A file or another non-directory filesystem entry.
    File,
}

/// A validated single directory name for a newly-created project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectName(String);

impl ProjectName {
    /// Validates and normalizes a project directory name.
    pub fn new(value: impl Into<String>) -> Result<Self, ProjectNameError> {
        let value = value.into();
        let name = value.trim();

        if name.is_empty() {
            return Err(ProjectNameError::Empty);
        }

        if matches!(name, "." | "..") || name.contains(['/', '\\']) {
            return Err(ProjectNameError::NotSingleComponent);
        }

        if let Some(character) = name
            .chars()
            .find(|character| character.is_control() || "<>:\"|?*".contains(*character))
        {
            return Err(ProjectNameError::InvalidCharacter(character));
        }

        if name.ends_with('.') {
            return Err(ProjectNameError::TrailingPeriod);
        }

        let device_name = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
        let is_reserved = matches!(device_name.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || device_name
                .strip_prefix("COM")
                .or_else(|| device_name.strip_prefix("LPT"))
                .is_some_and(|number| {
                    matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
                });

        if is_reserved {
            return Err(ProjectNameError::Reserved(name.to_owned()));
        }

        Ok(Self(name.to_owned()))
    }

    /// Returns the validated directory name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a proposed project directory name is unsafe or invalid.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProjectNameError {
    /// The name contained no visible characters.
    #[error("project name cannot be empty")]
    Empty,
    /// The name described more than one path component.
    #[error("project name must be a single folder name")]
    NotSingleComponent,
    /// The name included a character that is not portable across supported systems.
    #[error("project name contains invalid character '{0}'")]
    InvalidCharacter(char),
    /// Windows removes trailing periods from directory names.
    #[error("project name cannot end with a period")]
    TrailingPeriod,
    /// The name is reserved by Windows.
    #[error("'{0}' is a reserved project name")]
    Reserved(String),
}
