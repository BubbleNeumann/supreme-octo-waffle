//! Reading and writing the order an author has given a directory's documents.
//!
//! The order lives in one file of Lantern's own state, `.lantern/order.toml`,
//! as a table of project-relative directory paths against the document names
//! that directory holds, in the author's order:
//!
//! ```toml
//! "chapters" = ["two.md", "one.md"]
//! "chapters/act-one" = ["arrival.md"]
//! ```
//!
//! Separators are written as `/` whatever the system wrote them with, so that a
//! project carried from Windows to Linux keeps the order it was given.
//!
//! Nothing here is authoritative. The documents are the manuscript; this file
//! only says what sequence to draw them in, and a project whose order file is
//! missing, unreadable or nonsense opens and lists exactly as it would have
//! before an order was ever recorded.

use crate::StoreError;
use lantern_core::Project;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// The directory reserved for Lantern-owned state inside a project.
const LANTERN_DIRECTORY: &str = ".lantern";

/// The file the document order is recorded in.
const ORDER_FILE: &str = "order.toml";

/// Reads the order recorded for one directory's documents.
///
/// An order that cannot be read is no order: a missing file, a file that is not
/// TOML, and an entry that is not an array of strings all give an empty order
/// rather than a failure. Principle 4 asks that Lantern's own state never be
/// required to open a project, and an author whose order file is damaged should
/// lose the sequence, not the manuscript.
pub(crate) fn read(project: &Project, directory: &Path) -> Vec<String> {
    let Some(key) = order_key(directory) else {
        return Vec::new();
    };

    let Ok(text) = fs::read_to_string(order_path(project)) else {
        return Vec::new();
    };

    let Ok(document) = text.parse::<toml::Table>() else {
        return Vec::new();
    };

    let Some(names) = document.get(&key).and_then(toml::Value::as_array) else {
        return Vec::new();
    };

    names
        .iter()
        .filter_map(|name| name.as_str().map(str::to_owned))
        .collect()
}

/// Records the order of one directory's documents, replacing what was there.
///
/// An empty order removes the directory's entry rather than storing a hollow
/// one, so that a directory returned to its listed order leaves nothing behind.
/// A directory or document whose name is not UTF-8 cannot be written to a TOML
/// file, and is left unordered rather than mangled into one that would not
/// resolve.
pub(crate) fn write(
    project: &Project,
    directory: &Path,
    names: &[String],
) -> Result<(), StoreError> {
    let Some(key) = order_key(directory) else {
        return Ok(());
    };

    let path = order_path(project);
    let mut document = fs::read_to_string(&path)
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .unwrap_or_default();

    if names.is_empty() {
        document.remove(&key);
    } else {
        document.insert(
            key,
            toml::Value::Array(names.iter().map(|name| name.as_str().into()).collect()),
        );
    }

    let state_directory = lantern_directory(project);

    fs::create_dir_all(&state_directory).map_err(|source| StoreError::Io {
        path: state_directory.clone(),
        source,
    })?;

    let text = toml::to_string(&document).map_err(|_| StoreError::OrderNotWritable)?;

    // Written beside the file and renamed over it, so that an interrupted write
    // leaves the previous order rather than a half-written one.
    let mut temporary =
        tempfile::NamedTempFile::new_in(&state_directory).map_err(|source| StoreError::Io {
            path: state_directory,
            source,
        })?;

    temporary
        .write_all(text.as_bytes())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|source| StoreError::Io {
            path: path.clone(),
            source,
        })?;

    temporary.persist(&path).map_err(|error| StoreError::Io {
        path,
        source: error.error,
    })?;

    Ok(())
}

/// Returns the path of the project's order file.
fn order_path(project: &Project) -> PathBuf {
    lantern_directory(project).join(ORDER_FILE)
}

/// Returns the project's own state directory.
///
/// Built here rather than resolved as a project-relative path, because storage
/// refuses `.lantern` to every caller working on the author's behalf. This is
/// Lantern writing its own state, which is what that directory is reserved for.
fn lantern_directory(project: &Project) -> PathBuf {
    project.root().join(LANTERN_DIRECTORY)
}

/// Returns the table key a directory is recorded under.
///
/// The key spells the path with `/` on every system, and the project root - the
/// directory with no components at all - is spelled as the empty key. A path
/// that is not UTF-8 has no key, because a TOML file cannot hold one.
fn order_key(directory: &Path) -> Option<String> {
    let mut key = String::new();

    for component in directory.components() {
        let Component::Normal(name) = component else {
            return None;
        };

        if !key.is_empty() {
            key.push('/');
        }

        key.push_str(name.to_str()?);
    }

    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_a_nested_directory_with_forward_slashes() {
        let key = order_key(&Path::new("chapters").join("act-one")).expect("key");

        assert_eq!(key, "chapters/act-one");
    }

    #[test]
    fn spells_the_project_root_as_the_empty_key() {
        assert_eq!(order_key(Path::new("")), Some(String::new()));
    }

    #[test]
    fn refuses_a_path_that_is_not_a_plain_sequence_of_names() {
        assert_eq!(order_key(Path::new("../elsewhere")), None);
    }
}
