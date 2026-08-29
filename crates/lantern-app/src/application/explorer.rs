//! Project explorer state: which directories are listed, which are expanded,
//! and the flattened rows the sidebar draws.

use lantern_service::ProjectEntry;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Cached directory listings and expansion state for the project explorer.
///
/// Listings are only retained for directories the explorer is currently
/// showing. Collapsing a directory releases every listing underneath it while
/// keeping the expansion markers below, so re-expanding restores the same shape
/// from freshly read listings instead of holding a session's whole browsing
/// history in memory.
#[derive(Debug, Default)]
pub(crate) struct Explorer {
    listings: HashMap<PathBuf, Vec<ProjectEntry>>,
    expanded: HashSet<PathBuf>,
}

impl Explorer {
    /// Creates an explorer with no project loaded.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Replaces all state with a newly opened project's root listing.
    pub(crate) fn reset(&mut self, root_entries: Vec<ProjectEntry>) {
        self.clear();
        self.listings.insert(PathBuf::new(), root_entries);
    }

    /// Forgets every listing and expansion.
    pub(crate) fn clear(&mut self) {
        self.listings.clear();
        self.expanded.clear();
    }

    /// Returns whether the project root holds no entries.
    pub(crate) fn is_empty(&self) -> bool {
        self.listings
            .get(Path::new(""))
            .is_none_or(|entries| entries.is_empty())
    }

    /// Returns whether a directory is currently expanded.
    pub(crate) fn is_expanded(&self, directory: &Path) -> bool {
        self.expanded.contains(directory)
    }

    /// Returns whether a directory's listing is currently held in memory.
    ///
    /// Releasing listings is not otherwise observable — a collapsed directory
    /// draws the same either way — so this exists for the tests that cover it.
    #[cfg(test)]
    pub(crate) fn has_listing(&self, directory: &Path) -> bool {
        self.listings.contains_key(directory)
    }

    /// Marks a directory expanded without reading anything.
    ///
    /// The listing it needs is loaded separately, because the explorer does not
    /// perform persistence itself.
    pub(crate) fn expand(&mut self, directory: PathBuf) {
        self.expanded.insert(directory);
    }

    /// Collapses a directory and releases the listings it was showing.
    ///
    /// Expansion markers below the directory survive, so re-expanding it
    /// restores the previous shape once those listings are read again.
    pub(crate) fn collapse(&mut self, directory: &Path) {
        self.expanded.remove(directory);
        self.listings
            .retain(|listed, _| !listed.starts_with(directory));
    }

    /// Stores one directory's listing.
    pub(crate) fn insert_listing(&mut self, directory: PathBuf, entries: Vec<ProjectEntry>) {
        self.listings.insert(directory, entries);
    }

    /// Returns the next visible directory that is expanded but has no listing.
    ///
    /// Collapsing releases listings, so re-expanding a directory has to read the
    /// surviving expanded subtree back one directory at a time. Returns `None`
    /// once every visible directory is loaded.
    pub(crate) fn next_unlisted_directory(&self) -> Option<PathBuf> {
        self.find_unlisted(Path::new(""))
    }

    fn find_unlisted(&self, directory: &Path) -> Option<PathBuf> {
        for entry in self.listings.get(directory)? {
            let child = entry.relative_path();

            if !entry.is_directory() || !self.expanded.contains(child) {
                continue;
            }

            if !self.listings.contains_key(child) {
                return Some(child.to_owned());
            }

            if let Some(unlisted) = self.find_unlisted(child) {
                return Some(unlisted);
            }
        }

        None
    }

    /// Returns the flattened rows the sidebar draws, top to bottom.
    ///
    /// Rows borrow the listings rather than copying them, so the drawn tree
    /// costs one vector of references per frame instead of a second copy of
    /// every entry.
    pub(crate) fn visible_rows(&self) -> Vec<ExplorerRow<'_>> {
        let mut rows = Vec::new();
        self.append_rows(Path::new(""), 0, &mut rows);

        rows
    }

    fn append_rows<'a>(&'a self, directory: &Path, depth: usize, rows: &mut Vec<ExplorerRow<'a>>) {
        let Some(entries) = self.listings.get(directory) else {
            return;
        };

        for entry in entries {
            let expanded = entry.is_directory() && self.expanded.contains(entry.relative_path());

            rows.push(ExplorerRow {
                entry,
                depth,
                expanded,
            });

            if expanded {
                self.append_rows(entry.relative_path(), depth + 1, rows);
            }
        }
    }
}

/// One drawn row of the project explorer.
#[derive(Debug)]
pub(crate) struct ExplorerRow<'a> {
    /// The listed entry this row draws.
    pub(crate) entry: &'a ProjectEntry,
    /// How far the entry sits below the project root.
    pub(crate) depth: usize,
    /// Whether this row is a directory that is currently expanded.
    pub(crate) expanded: bool,
}
