//! Domain types and invariants shared across Lantern.

pub mod theme;

pub use theme::{BaseColors, Color, Theme, ThemeError, ThemeMode, ThemePalette};

use std::path::{Path, PathBuf};
use thiserror::Error;

/// The directories a Lantern project keeps at its root, in the order shown.
///
/// A project opens onto these and nothing else, so that every project presents
/// the same shape however the directory behind it is arranged. Anything else at
/// the root belongs to the author rather than to Lantern, and stays out of
/// sight rather than being moved or removed.
pub const WORKSPACE_DIRECTORIES: [&str; 3] = [DEFAULT_DOCUMENT_DIRECTORY, "references", "drawer"];

/// The workspace directory a document is created in when nothing is open.
///
/// A project is opened to draft in, so a document created with no document to
/// sit beside goes where the drafts are kept.
pub const DEFAULT_DOCUMENT_DIRECTORY: &str = "chapters";

/// The extension given to a document created without one.
pub const DEFAULT_DOCUMENT_EXTENSION: &str = "md";

/// The file extensions Lantern opens in the editor, lowercased.
pub const EDITABLE_EXTENSIONS: [&str; 3] = [DEFAULT_DOCUMENT_EXTENSION, "markdown", "txt"];

/// Returns whether a path names a document Lantern can open in the editor.
///
/// The extension is compared without case, because a file saved as `CHAPTER.MD`
/// on a system that reports it that way is the same kind of document as one
/// saved as `chapter.md`.
pub fn has_editable_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            EDITABLE_EXTENSIONS
                .iter()
                .any(|editable| extension.eq_ignore_ascii_case(editable))
        })
}

/// Returns one directory's entries with its documents in the order given.
///
/// Directories keep their place ahead of the documents, in the order storage
/// listed them; an author orders a manuscript, not the shelves it sits on.
/// Documents `order` names come first, in the order it names them, and any it
/// does not name follow in the order they arrived in. A document added outside
/// Lantern is therefore drawn at the end rather than not at all, and an order
/// naming documents that have since been deleted simply has nothing to place.
pub fn order_documents(entries: Vec<ProjectEntry>, order: &[String]) -> Vec<ProjectEntry> {
    let (mut directories, mut documents): (Vec<_>, Vec<_>) =
        entries.into_iter().partition(ProjectEntry::is_directory);

    // A stable sort, so documents the order does not name keep the sequence
    // storage gave them rather than being shuffled among themselves.
    documents.sort_by_key(|entry| {
        order
            .iter()
            .position(|name| name == entry.name())
            .unwrap_or(usize::MAX)
    });

    directories.append(&mut documents);

    directories
}

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

/// The line terminator a document uses on disk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LineEnding {
    /// A single line feed, used by Unix-like systems.
    #[default]
    Lf,
    /// A carriage return followed by a line feed, used by Windows.
    Crlf,
}

impl LineEnding {
    /// Returns the terminator's characters.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// The byte-level conventions a document uses on disk.
///
/// Lantern normalizes documents for editing, so these conventions are recorded
/// separately and restored on save. Without them, opening and saving a file
/// authored on another platform would rewrite every line in it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DocumentEncoding {
    line_ending: LineEnding,
    byte_order_mark: bool,
}

impl DocumentEncoding {
    /// Splits raw file text into its on-disk conventions and editable content.
    ///
    /// A leading byte order mark is removed and CRLF terminators become LF. The
    /// terminator is taken from the first line break in the file, so a file with
    /// mixed terminators is normalized to whichever convention it opens with.
    pub fn detect(raw_content: &str) -> (Self, String) {
        let (byte_order_mark, text) = match raw_content.strip_prefix('\u{feff}') {
            Some(text) => (true, text),
            None => (false, raw_content),
        };

        let line_ending = match text.find('\n') {
            Some(index) if text[..index].ends_with('\r') => LineEnding::Crlf,
            _ => LineEnding::Lf,
        };

        let content = match line_ending {
            LineEnding::Lf => text.to_owned(),
            LineEnding::Crlf => text.replace("\r\n", "\n"),
        };

        (
            Self {
                line_ending,
                byte_order_mark,
            },
            content,
        )
    }

    /// Restores the on-disk conventions around normalized editor text.
    pub fn apply(&self, content: &str) -> String {
        let body = match self.line_ending {
            LineEnding::Lf => content.replace("\r\n", "\n"),
            LineEnding::Crlf => content.replace("\r\n", "\n").replace('\n', "\r\n"),
        };

        if self.byte_order_mark {
            let mut raw_content = String::with_capacity(body.len() + '\u{feff}'.len_utf8());
            raw_content.push('\u{feff}');
            raw_content.push_str(&body);
            return raw_content;
        }

        body
    }

    /// Returns the document's line terminator.
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Returns whether the document begins with a byte order mark.
    pub fn has_byte_order_mark(&self) -> bool {
        self.byte_order_mark
    }
}

/// An opened human-authored text document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    relative_path: PathBuf,
    content: String,
    encoding: DocumentEncoding,
}

impl Document {
    /// Creates a document from raw file text and a project-relative path verified by storage.
    ///
    /// The text is normalized for editing and the file's original conventions are
    /// retained so that saving reproduces them.
    pub fn from_verified_content(relative_path: PathBuf, raw_content: String) -> Self {
        let (encoding, content) = DocumentEncoding::detect(&raw_content);

        Self {
            relative_path,
            content,
            encoding,
        }
    }

    /// Returns the document's path relative to its project root.
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Returns the normalized document text.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the conventions to restore when saving this document.
    pub fn encoding(&self) -> DocumentEncoding {
        self.encoding
    }

    /// Returns whether `content` differs from the text this document holds.
    ///
    /// The document's text mirrors what was last read from or written to
    /// storage, so this answers whether saving `content` would change the file.
    pub fn differs_from(&self, content: &str) -> bool {
        self.content != content
    }

    /// Adopts a path that storage has moved this document to.
    ///
    /// Moving changes nothing else about a document: the text, its unsaved
    /// edits and the encoding to restore are the same ones. Without this the
    /// in-memory copy would go on naming the path the file has left, and the
    /// next save would be written where nothing is any more.
    pub fn record_moved(&mut self, relative_path: PathBuf) {
        self.relative_path = relative_path;
    }

    /// Adopts text that storage has written as the document's own content.
    ///
    /// Saving does not otherwise change a document, so without this the
    /// in-memory copy would drift from the file it describes.
    pub fn record_saved(&mut self, content: String) {
        self.content = content;
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
    ///
    /// `name` is a display string and may be lossy; `relative_path` must be the
    /// exact path storage can resolve again.
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

    /// Returns the entry's file name for display.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this entry is an ordinary directory.
    pub fn is_directory(&self) -> bool {
        self.kind == ProjectEntryKind::Directory
    }

    /// Returns whether this entry is a directory carrying `name`.
    ///
    /// The name is compared without case, because Windows and macOS report a
    /// directory as it was created rather than as it was asked for, and a
    /// project holding `Chapters` holds the chapters directory.
    pub fn is_directory_named(&self, name: &str) -> bool {
        self.is_directory() && self.name.eq_ignore_ascii_case(name)
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

        if let Err(problem) = check_name_component(name) {
            return Err(match problem {
                NameProblem::Empty => ProjectNameError::Empty,
                NameProblem::NotSingleComponent => ProjectNameError::NotSingleComponent,
                NameProblem::InvalidCharacter(character) => {
                    ProjectNameError::InvalidCharacter(character)
                }
                NameProblem::TrailingPeriod => ProjectNameError::TrailingPeriod,
                NameProblem::Reserved => ProjectNameError::Reserved(name.to_owned()),
            });
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

/// A validated single file name for a newly-created document.
///
/// The name always ends in one of [`EDITABLE_EXTENSIONS`], so a document that
/// Lantern creates is a document Lantern can open again. A name that does not
/// already carry one is given [`DEFAULT_DOCUMENT_EXTENSION`] rather than
/// refused: titles hold periods, and `Mrs. Dalloway` is a chapter rather than a
/// file of some unknown kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentName(String);

impl DocumentName {
    /// Validates a document file name, giving it an editable extension.
    pub fn new(value: impl Into<String>) -> Result<Self, DocumentNameError> {
        let value = value.into();
        let name = value.trim();

        if let Err(problem) = check_name_component(name) {
            return Err(match problem {
                NameProblem::Empty => DocumentNameError::Empty,
                NameProblem::NotSingleComponent => DocumentNameError::NotSingleComponent,
                NameProblem::InvalidCharacter(character) => {
                    DocumentNameError::InvalidCharacter(character)
                }
                NameProblem::TrailingPeriod => DocumentNameError::TrailingPeriod,
                NameProblem::Reserved => DocumentNameError::Reserved(name.to_owned()),
            });
        }

        if has_editable_extension(Path::new(name)) {
            return Ok(Self(name.to_owned()));
        }

        Ok(Self(format!("{name}.{DEFAULT_DOCUMENT_EXTENSION}")))
    }

    /// Returns the validated file name, extension included.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a proposed document file name is unsafe or invalid.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DocumentNameError {
    /// The name contained no visible characters.
    #[error("document name cannot be empty")]
    Empty,
    /// The name described more than one path component.
    #[error("document name must be a single file name")]
    NotSingleComponent,
    /// The name included a character that is not portable across supported systems.
    #[error("document name contains invalid character '{0}'")]
    InvalidCharacter(char),
    /// Windows removes trailing periods from file names.
    #[error("document name cannot end with a period")]
    TrailingPeriod,
    /// The name is reserved by Windows.
    #[error("'{0}' is a reserved document name")]
    Reserved(String),
}

/// Why a proposed single path component is unsafe or invalid.
///
/// Named separately from the public errors because the same rules govern
/// project directories and documents, while the wording an author sees does
/// not.
enum NameProblem {
    /// The name contained no visible characters.
    Empty,
    /// The name described more than one path component.
    NotSingleComponent,
    /// The name included a character that is not portable across supported systems.
    InvalidCharacter(char),
    /// Windows removes trailing periods from names.
    TrailingPeriod,
    /// The name is reserved by Windows.
    Reserved,
}

/// Checks a trimmed name against the rules for one filesystem component.
///
/// The rules are the strictest of the supported systems rather than those of
/// the one Lantern happens to be running on, so that a project created on Linux
/// is a project that opens on Windows.
fn check_name_component(name: &str) -> Result<(), NameProblem> {
    if name.is_empty() {
        return Err(NameProblem::Empty);
    }

    if matches!(name, "." | "..") || name.contains(['/', '\\']) {
        return Err(NameProblem::NotSingleComponent);
    }

    if let Some(character) = name
        .chars()
        .find(|character| character.is_control() || "<>:\"|?*".contains(*character))
    {
        return Err(NameProblem::InvalidCharacter(character));
    }

    if name.ends_with('.') {
        return Err(NameProblem::TrailingPeriod);
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
        return Err(NameProblem::Reserved);
    }

    Ok(())
}
