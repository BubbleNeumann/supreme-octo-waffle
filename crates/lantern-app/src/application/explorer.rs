//! Project explorer state: which directories are listed, which are expanded,
//! and the flattened rows the sidebar draws.

use lantern_service::{ProjectEntry, is_chapter, scene_directory};
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

    /// Drops one directory's listing without collapsing it.
    ///
    /// A directory Lantern has just written into is stale in memory. Forgetting
    /// its listing is enough to have it read again, because a directory that is
    /// expanded without a listing is one of the directories loaded on the next
    /// pass; collapsing it would also close everything below.
    pub(crate) fn forget_listing(&mut self, directory: &Path) {
        self.listings.remove(directory);
    }

    /// Stores one directory's listing.
    pub(crate) fn insert_listing(&mut self, directory: PathBuf, entries: Vec<ProjectEntry>) {
        self.listings.insert(directory, entries);
    }

    /// Returns one directory's listing, when the explorer is holding it.
    ///
    /// The sidebar draws from [`Self::visible_rows`]; this answers questions
    /// about one directory on its own, such as which document follows another.
    pub(crate) fn listing(&self, directory: &Path) -> Option<&[ProjectEntry]> {
        self.listings.get(directory).map(Vec::as_slice)
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

        // A chapter is drawn as though it were the directory its scenes are
        // kept in, so that directory is not drawn beside it as well.
        let scene_directories: Vec<PathBuf> = entries
            .iter()
            .filter(|entry| !entry.is_directory() && is_chapter(entry.relative_path()))
            .filter_map(|entry| scene_directory(entry.relative_path()))
            .collect();
        let mut chapters = 0;

        for entry in entries {
            let chapter_number = if entry.is_directory() || !is_chapter(entry.relative_path()) {
                None
            } else {
                chapters += 1;
                Some(chapters)
            };

            let children = if entry.is_directory() {
                if scene_directories
                    .iter()
                    .any(|scenes| scenes == entry.relative_path())
                {
                    continue;
                }

                Some(entry.relative_path().to_owned())
            } else {
                // A chapter with no scene directory beside it has no children
                // to disclose; it is an ordinary document until one is dragged
                // under it.
                chapter_number
                    .and_then(|_| scene_directory(entry.relative_path()))
                    .filter(|directory| {
                        entries.iter().any(|listed| {
                            listed.is_directory() && listed.relative_path() == directory
                        })
                    })
            };

            let expanded = children
                .as_ref()
                .is_some_and(|children| self.expanded.contains(children));

            rows.push(ExplorerRow {
                entry,
                depth,
                expanded,
                children: children.clone(),
                chapter_number,
            });

            if let Some(children) = children.filter(|_| expanded) {
                self.append_rows(&children, depth + 1, rows);
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
    /// Whether this row's children are currently shown beneath it.
    pub(crate) expanded: bool,
    /// The directory holding the rows drawn under this one, when it has any.
    ///
    /// A directory's own path, or the scene directory of a chapter that has
    /// scenes. It is what expanding and collapsing this row acts on, and it is
    /// `None` for a row nothing can be drawn beneath.
    pub(crate) children: Option<PathBuf>,
    /// Which chapter this row draws, counting from the top of the directory.
    ///
    /// `None` for everything that is not a chapter. The number is the chapter's
    /// place in the order the author put it in rather than anything written
    /// down, so dragging a chapter renumbers the ones it passes.
    pub(crate) chapter_number: Option<usize>,
}
