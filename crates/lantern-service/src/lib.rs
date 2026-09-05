//! Application use-cases and business rules for Lantern.

use lantern_core::{
    DocumentName, DocumentNameError, ProjectName, ProjectNameError, has_editable_extension,
    join_scenes, order_documents, split_scenes, unused_scene_name,
};
use lantern_store::{ProjectStore, StoreError, ThemeStore};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use lantern_core::{
    Color, DEFAULT_DOCUMENT_DIRECTORY, Document, DocumentEncoding, LineEnding, Project,
    ProjectEntry, SCENE_SEPARATOR, Theme, ThemeMode, ThemePalette, WORKSPACE_DIRECTORIES,
    is_chapter, is_scene_directory_of, scene_directory,
};
pub use lantern_store::{FsProjectStore, FsThemeStore};

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
    ///
    /// The directories in [`WORKSPACE_DIRECTORIES`] are put in place as part of
    /// opening, so a project always presents the same root.
    pub fn open_project(&self, root: &Path) -> Result<Project, ProjectServiceError> {
        let project = self.store.open_project(root)?;

        self.create_workspace(&project)?;

        Ok(project)
    }

    /// Creates and opens a new project directory under an existing parent.
    pub fn create_project(
        &self,
        parent: &Path,
        name: impl Into<String>,
    ) -> Result<Project, ProjectServiceError> {
        let name = ProjectName::new(name)?;
        let project = self.store.create_project(parent, &name)?;

        self.create_workspace(&project)?;

        Ok(project)
    }

    /// Lists one project directory for the explorer.
    ///
    /// The root lists as the workspace directories alone, in the order
    /// [`WORKSPACE_DIRECTORIES`] declares. Everything else the author keeps
    /// beside them is left where it is and not shown. Directories below the
    /// root list in full.
    pub fn list_directory(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Vec<ProjectEntry>, ProjectServiceError> {
        let entries = self.store.list_directory(project, relative_path)?;

        if !relative_path.as_os_str().is_empty() {
            return Ok(order_documents(
                entries,
                &self.store.document_order(project, relative_path),
            ));
        }

        // The root's own order is fixed by the workspace it presents, so the
        // author's ordering does not reach it.

        Ok(WORKSPACE_DIRECTORIES
            .iter()
            .filter_map(|name| {
                entries
                    .iter()
                    .find(|entry| entry.is_directory_named(name))
                    .cloned()
            })
            .collect())
    }

    /// Puts the directories every project keeps in place, leaving others alone.
    fn create_workspace(&self, project: &Project) -> Result<(), ProjectServiceError> {
        for name in WORKSPACE_DIRECTORIES {
            self.store.create_directory(project, Path::new(name))?;
        }

        Ok(())
    }

    /// Creates an empty document inside one of a project's directories.
    ///
    /// The typed name is validated and given an editable extension, so that a
    /// document Lantern creates is one it can open again. A name already taken
    /// is a failure rather than a document emptied.
    pub fn create_document(
        &self,
        project: &Project,
        directory: &Path,
        name: impl Into<String>,
    ) -> Result<Document, ProjectServiceError> {
        let name = DocumentName::new(name)?;
        let relative_path = directory.join(name.as_str());

        self.store.create_document(project, &relative_path)?;

        // A document created inside a chapter's scene directory is a scene, and
        // a chapter is the scenes it holds.
        let chapter = self.chapter_of_scene(project, &relative_path);
        self.rebuild_chapters(project, &[chapter])?;

        Ok(self.store.read_document(project, &relative_path)?)
    }

    /// Moves a document into another of the project's directories.
    ///
    /// The document keeps its name; only the directory holding it changes.
    /// Returns the project-relative path it now has, which the caller needs in
    /// order to go on describing the document it already holds open.
    pub fn move_document(
        &self,
        project: &Project,
        relative_path: &Path,
        directory: &Path,
    ) -> Result<PathBuf, ProjectServiceError> {
        let source_chapter = self.chapter_of_scene(project, relative_path);

        self.open_scene_directory(project, directory)?;

        let moved_path = self
            .store
            .move_document(project, relative_path, directory)?;
        let destination_chapter = self.chapter_of_scene(project, &moved_path);

        self.rebuild_chapters(project, &[source_chapter, destination_chapter])?;

        Ok(moved_path)
    }

    /// Puts a document in a directory, in a place the author has chosen.
    ///
    /// The document is moved when it comes from another directory, and either
    /// way the directory's order is recorded with the document standing before
    /// `before` - or last, when no document is named. The whole resulting
    /// sequence is written, so a directory that had no order acquires the one
    /// it was being drawn in, with the placed document moved within it.
    ///
    /// Returns the project-relative path the document now has.
    pub fn place_document(
        &self,
        project: &Project,
        relative_path: &Path,
        directory: &Path,
        before: Option<&str>,
    ) -> Result<PathBuf, ProjectServiceError> {
        let source_chapter = self.chapter_of_scene(project, relative_path);
        let moved_path = if relative_path.parent() == Some(directory) {
            relative_path.to_owned()
        } else {
            self.open_scene_directory(project, directory)?;
            self.store
                .move_document(project, relative_path, directory)?
        };

        let Some(name) = moved_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            // A name that is not UTF-8 cannot be recorded. The document has
            // still moved; it simply keeps the place its listing gives it.
            let destination_chapter = self.chapter_of_scene(project, &moved_path);
            self.rebuild_chapters(project, &[source_chapter, destination_chapter])?;

            return Ok(moved_path);
        };

        // Ordering a document against itself asks for the sequence that is
        // already there.
        if before == Some(name.as_str()) {
            let destination_chapter = self.chapter_of_scene(project, &moved_path);
            self.rebuild_chapters(project, &[source_chapter, destination_chapter])?;

            return Ok(moved_path);
        }

        let mut order: Vec<String> = self
            .list_directory(project, directory)?
            .iter()
            .filter(|entry| !entry.is_directory())
            .map(|entry| entry.name().to_owned())
            .collect();

        order.retain(|listed| listed != &name);

        match before.and_then(|before| order.iter().position(|listed| listed == before)) {
            Some(index) => order.insert(index, name),
            None => order.push(name),
        }

        self.store.set_document_order(project, directory, &order)?;

        // After the order is recorded, because a chapter reads its scenes in it.
        let destination_chapter = self.chapter_of_scene(project, &moved_path);
        self.rebuild_chapters(project, &[source_chapter, destination_chapter])?;

        Ok(moved_path)
    }

    /// Returns the scenes a chapter is written in, in the author's order.
    ///
    /// A chapter with no scene directory beside it has none, which is every
    /// chapter until a document is dragged under one: such a chapter is an
    /// ordinary document, and its file is the whole of it. A scene directory
    /// that cannot be listed reads the same way, so that a chapter stays
    /// editable whatever state the directory beside it is in.
    pub fn scenes(&self, project: &Project, chapter: &Path) -> Vec<PathBuf> {
        if !is_chapter(chapter) {
            return Vec::new();
        }

        let Some(directory) = scene_directory(chapter) else {
            return Vec::new();
        };

        let Ok(entries) = self.list_directory(project, &directory) else {
            return Vec::new();
        };

        entries
            .into_iter()
            .filter(|entry| !entry.is_directory() && has_editable_extension(entry.relative_path()))
            .map(|entry| entry.relative_path().to_owned())
            .collect()
    }

    /// Returns the chapter whose scenes belong in `directory`, if there is one.
    ///
    /// The chapter is looked for in the listing rather than derived from the
    /// path, because the directory carries the chapter's name and not which of
    /// the editable formats it is written in.
    fn chapter_holding(&self, project: &Project, directory: &Path) -> Option<PathBuf> {
        let parent = directory.parent()?;

        self.list_directory(project, parent)
            .ok()?
            .into_iter()
            .map(|entry| entry.relative_path().to_owned())
            .find(|path| is_scene_directory_of(directory, path))
    }

    /// Returns the chapter a document is a scene of, if it is one.
    fn chapter_of_scene(&self, project: &Project, relative_path: &Path) -> Option<PathBuf> {
        self.chapter_holding(project, relative_path.parent()?)
    }

    /// Puts a chapter's scene directory in place before a scene arrives in it.
    ///
    /// Returns the chapter the directory belongs to, or `None` when it is an
    /// ordinary directory that is already there and needs nothing done to it.
    fn open_scene_directory(
        &self,
        project: &Project,
        directory: &Path,
    ) -> Result<Option<PathBuf>, ProjectServiceError> {
        let Some(chapter) = self.chapter_holding(project, directory) else {
            return Ok(None);
        };

        self.store.create_directory(project, directory)?;
        self.keep_chapter_text_as_a_scene(project, &chapter)?;

        Ok(Some(chapter))
    }

    /// Moves a chapter's own text into a scene before it gains its first one.
    ///
    /// A chapter that has scenes is written by them, so a chapter that held
    /// prose of its own would have that prose replaced by the arriving scene's.
    /// Instead the text it already had becomes the first scene under it, named
    /// after the chapter, and dragging a scene under a chapter costs an author
    /// nothing they wrote.
    fn keep_chapter_text_as_a_scene(
        &self,
        project: &Project,
        chapter: &Path,
    ) -> Result<(), ProjectServiceError> {
        if !self.scenes(project, chapter).is_empty() {
            return Ok(());
        }

        let document = self.store.read_document(project, chapter)?;

        if document.content().trim().is_empty() {
            return Ok(());
        }

        let (Some(directory), Some(name)) = (
            scene_directory(chapter),
            chapter.file_name().and_then(|name| name.to_str()),
        ) else {
            return Ok(());
        };

        let scene = directory.join(name);

        self.store.create_document(project, &scene)?;
        self.store.save_document(
            project,
            &scene,
            &document.encoding().apply(document.content()),
        )?;

        // Recorded as the order rather than left to sort by name, so that the
        // scene arriving next stands after the chapter's own text wherever its
        // name would otherwise have put it.
        self.store
            .set_document_order(project, &directory, &[name.to_owned()])?;

        Ok(())
    }

    /// Writes each named chapter's file as the scenes beneath it, joined.
    ///
    /// Takes the chapters as options, and tolerates the same chapter twice,
    /// because that is what the callers have: the chapter a document left and
    /// the chapter it arrived in, either of which may be no chapter at all.
    fn rebuild_chapters(
        &self,
        project: &Project,
        chapters: &[Option<PathBuf>],
    ) -> Result<(), ProjectServiceError> {
        let mut rebuilt: Vec<&Path> = Vec::new();

        for chapter in chapters.iter().flatten() {
            if rebuilt.contains(&chapter.as_path()) {
                continue;
            }

            rebuilt.push(chapter);
            self.rebuild_chapter(project, chapter)?;
        }

        Ok(())
    }

    /// Writes one chapter's file as the scenes beneath it, joined.
    ///
    /// A chapter with no scenes is left exactly as it is. Its file is the whole
    /// chapter, there is nothing to derive it from, and emptying it would be a
    /// strange answer to the last scene being dragged out from under it.
    fn rebuild_chapter(
        &self,
        project: &Project,
        chapter: &Path,
    ) -> Result<(), ProjectServiceError> {
        let scenes = self.scenes(project, chapter);

        if scenes.is_empty() {
            return Ok(());
        }

        let mut texts = Vec::with_capacity(scenes.len());

        for scene in &scenes {
            texts.push(self.store.read_document(project, scene)?);
        }

        let content = join_scenes(texts.iter().map(Document::content));
        let document = self.store.read_document(project, chapter)?;

        // A chapter that already reads as its scenes is not rewritten, so that
        // saving a scene that did not change leaves every file's date alone.
        if !document.differs_from(&content) {
            return Ok(());
        }

        self.store
            .save_document(project, chapter, &document.encoding().apply(&content))?;

        Ok(())
    }

    /// Writes a chapter's text back out over the scenes it is written in.
    ///
    /// The chapter is split at its separators and the pieces go to the scenes in
    /// order. An author who added a separator has written another scene, and it
    /// is created; one who removed a separator has joined two, and the scene
    /// left over is deleted - the words that were in it are in the scene above,
    /// which is what joining them meant.
    ///
    /// A chapter with no scenes has nothing to distribute and is left alone.
    fn distribute_chapter(
        &self,
        project: &Project,
        chapter: &Path,
        content: &str,
    ) -> Result<(), ProjectServiceError> {
        let scenes = self.scenes(project, chapter);

        if scenes.is_empty() {
            return Ok(());
        }

        let Some(directory) = scene_directory(chapter) else {
            return Ok(());
        };

        let segments = split_scenes(content);

        for (scene, segment) in scenes.iter().zip(&segments) {
            let document = self.store.read_document(project, scene)?;

            if !document.differs_from(segment) {
                continue;
            }

            self.store
                .save_document(project, scene, &document.encoding().apply(segment))?;
        }

        if segments.len() == scenes.len() {
            return Ok(());
        }

        let kept = scenes.len().min(segments.len());
        let mut names: Vec<String> = scenes
            .iter()
            .take(kept)
            .filter_map(|scene| scene.file_name().and_then(|name| name.to_str()))
            .map(str::to_owned)
            .collect();
        // A scene whose name is not UTF-8 cannot be written into an order, and
        // an order missing one of the scenes would move that scene rather than
        // leave it where it is. Such a directory keeps the order it had.
        let recordable = names.len() == kept;

        for scene in scenes.iter().skip(kept) {
            self.store.delete_document(project, scene)?;
        }

        for segment in segments.iter().skip(kept) {
            let name = unused_scene_name(&names);
            let scene = directory.join(&name);

            // A scene Lantern creates is a new file, so it has no conventions of
            // its own to restore; the chapter's text goes in as it stands.
            self.store.create_document(project, &scene)?;
            self.store.save_document(project, &scene, segment)?;

            names.push(name);
        }

        if recordable {
            self.store.set_document_order(project, &directory, &names)?;
        }

        Ok(())
    }

    /// Opens a supported Markdown or plain-text document.
    pub fn open_document(
        &self,
        project: &Project,
        relative_path: &Path,
    ) -> Result<Document, ProjectServiceError> {
        if !has_editable_extension(relative_path) {
            return Err(ProjectServiceError::UnsupportedDocument(
                relative_path.to_owned(),
            ));
        }

        Ok(self.store.read_document(project, relative_path)?)
    }

    /// Saves edited text back over an open document.
    ///
    /// The document's original line endings and byte order mark are restored, so
    /// that saving an unmodified buffer reproduces the file byte for byte. A
    /// document that saves successfully adopts `content` as its own text, so
    /// that it keeps describing the file that is now on disk.
    pub fn save_document(
        &self,
        project: &Project,
        document: &mut Document,
        content: &str,
    ) -> Result<(), ProjectServiceError> {
        let relative_path = document.relative_path().to_owned();

        if !has_editable_extension(&relative_path) {
            return Err(ProjectServiceError::UnsupportedDocument(relative_path));
        }

        self.store
            .save_document(project, &relative_path, &document.encoding().apply(content))?;
        document.record_saved(content.to_owned());

        // A chapter and its scenes are two readings of the same words, so
        // whichever of them was written, the other follows.
        if is_chapter(&relative_path) {
            return self.distribute_chapter(project, &relative_path, content);
        }

        let chapter = self.chapter_of_scene(project, &relative_path);

        self.rebuild_chapters(project, &[chapter])
    }
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
    /// The requested document name violated a domain invariant.
    #[error(transparent)]
    InvalidDocumentName(#[from] DocumentNameError),
    /// The selected file is not an editable MVP document format.
    #[error("'{}' is not an editable Markdown or text document", .0.display())]
    UnsupportedDocument(PathBuf),
    /// The persistence adapter could not complete the operation.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Interface theme use-cases over an injected theme adapter.
#[derive(Debug)]
pub struct ThemeService<S> {
    store: S,
}

impl<S> ThemeService<S> {
    /// Creates a service using the provided theme adapter.
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S: ThemeStore> ThemeService<S> {
    /// Lists the themes the interface can offer, ordered by name.
    pub fn available_themes(&self) -> Result<Vec<Theme>, ThemeServiceError> {
        Ok(self.store.list_themes()?)
    }

    /// Loads the theme with the given name.
    pub fn theme(&self, name: &str) -> Result<Theme, ThemeServiceError> {
        self.available_themes()?
            .into_iter()
            .find(|theme| theme.name() == name)
            .ok_or_else(|| ThemeServiceError::UnknownTheme(name.to_owned()))
    }
}

impl ThemeService<FsThemeStore> {
    /// Creates the desktop service backed by theme files on disk.
    pub fn filesystem(search_paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self::new(FsThemeStore::new(search_paths))
    }
}

/// The desktop client's file-backed theme service type.
pub type FsThemeService = ThemeService<FsThemeStore>;

/// A failure while loading an interface theme.
#[derive(Debug, Error)]
pub enum ThemeServiceError {
    /// No installed theme carries the requested name.
    #[error("no theme named '{0}' is installed")]
    UnknownTheme(String),
    /// The theme adapter could not read the installed themes.
    #[error(transparent)]
    Store(#[from] StoreError),
}
