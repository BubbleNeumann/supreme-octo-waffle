//! Chapters, and the scenes a chapter is written in.
//!
//! A chapter is an ordinary document directly inside a project's chapters
//! directory. Beside it there may be a directory carrying its name, and the
//! documents in that directory are the chapter's scenes:
//!
//! ```text
//! chapters/
//!     Arrival.md          the chapter
//!     Arrival/            its scenes
//!         The station.md
//!         The house.md
//! ```
//!
//! The chapter's file holds its scenes' text, joined by a line of two minus
//! signs. Both files are the author's, both are readable on their own, and
//! either one can be edited: what this module provides is the pair of pure
//! operations that keep them saying the same thing.

use crate::{DEFAULT_DOCUMENT_DIRECTORY, DEFAULT_DOCUMENT_EXTENSION, has_editable_extension};
use std::path::{Component, Path, PathBuf};

/// The text that stands between two scenes inside a chapter's file.
///
/// A line of its own holding two minus signs. It is written into a document an
/// author reads and edits, so it has to be something they can type back.
pub const SCENE_SEPARATOR: &str = "\n--\n";

/// Returns whether a project-relative path names a chapter.
///
/// A chapter is an editable document lying directly in the chapters directory.
/// Nothing else is: a document one level further down is a scene, and a
/// document in `references` or `drawer` is neither. The directory's name is
/// compared without case, because Windows and macOS report a directory as it
/// was created rather than as it was asked for.
pub fn is_chapter(relative_path: &Path) -> bool {
    if !has_editable_extension(relative_path) {
        return false;
    }

    let Some(parent) = relative_path.parent() else {
        return false;
    };

    let mut components = parent.components();

    let Some(Component::Normal(name)) = components.next() else {
        return false;
    };

    components.next().is_none()
        && name
            .as_encoded_bytes()
            .eq_ignore_ascii_case(DEFAULT_DOCUMENT_DIRECTORY.as_bytes())
}

/// Returns the directory a chapter's scenes are kept in.
///
/// The directory carries the chapter's own name without its extension and
/// stands beside it, so that a project browsed outside Lantern reads as what it
/// is. `None` for a path with no name to take, which is not a chapter anyway.
pub fn scene_directory(chapter: &Path) -> Option<PathBuf> {
    let stem = chapter.file_stem()?;
    let parent = chapter.parent().unwrap_or(Path::new(""));

    Some(parent.join(stem))
}

/// Returns whether a directory holds the scenes of a particular chapter.
pub fn is_scene_directory_of(directory: &Path, chapter: &Path) -> bool {
    is_chapter(chapter) && scene_directory(chapter).as_deref() == Some(directory)
}

/// Joins scenes into the text of the chapter that holds them.
pub fn join_scenes<'a>(scenes: impl IntoIterator<Item = &'a str>) -> String {
    scenes.into_iter().collect::<Vec<_>>().join(SCENE_SEPARATOR)
}

/// Splits a chapter's text back into the scenes it is written in.
///
/// The inverse of [`join_scenes`] for any text it produced, so a chapter that
/// is split and rejoined is the same chapter byte for byte. Text holding no
/// separator is one scene, and empty text is one empty scene rather than none:
/// a chapter always has at least the scene the author is writing.
pub fn split_scenes(content: &str) -> Vec<&str> {
    content.split(SCENE_SEPARATOR).collect()
}

/// Returns a scene file name that no name in `taken` already carries.
///
/// Scenes an author drags in keep the names they were given; this names the
/// ones Lantern creates for itself, when a chapter is edited into holding more
/// scenes than there were files for. Names are compared without case, because a
/// system that treats `Scene 1.md` and `scene 1.md` as one file would refuse
/// the second.
pub fn unused_scene_name(taken: &[String]) -> String {
    let mut number = 1usize;

    loop {
        let name = format!("Scene {number}.{DEFAULT_DOCUMENT_EXTENSION}");

        if !taken
            .iter()
            .any(|name_taken| name_taken.eq_ignore_ascii_case(&name))
        {
            return name;
        }

        number += 1;
    }
}
