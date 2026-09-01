use super::*;
use lantern_service::WORKSPACE_DIRECTORIES;

/// Returns the names the explorer draws at the project root.
fn workspace_names(lantern: &Lantern) -> Vec<&str> {
    lantern
        .explorer
        .visible_rows()
        .into_iter()
        .filter(|row| row.depth == 0)
        .map(|row| row.entry.name())
        .collect()
}

#[test]
fn edit_message_updates_the_editor_buffer() {
    let (mut lantern, _) = boot();

    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Edit(text_editor::Edit::Insert('L'))),
    ));

    assert_eq!(lantern.editor.text(), "L");
}

#[test]
fn toggle_sidebar_message_changes_its_visibility() {
    let (mut lantern, _) = boot();

    drop(update(&mut lantern, Message::ToggleSidebar));
    assert!(lantern.sidebar_collapsed);

    drop(update(&mut lantern, Message::ToggleSidebar));
    assert!(!lantern.sidebar_collapsed);
}

#[test]
fn control_and_mouse_wheel_adjust_the_editor_font_size() {
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::ModifiersChanged(keyboard::Modifiers::CTRL),
    ));

    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Scroll { lines: -4 }),
    ));
    assert_eq!(lantern.editor_font_size, DEFAULT_EDITOR_FONT_SIZE + 1.0);

    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Scroll { lines: 4 }),
    ));
    assert_eq!(lantern.editor_font_size, DEFAULT_EDITOR_FONT_SIZE);
}

#[test]
fn editor_font_size_stays_within_readable_limits() {
    let (mut lantern, _) = boot();
    lantern.modifiers = keyboard::Modifiers::CTRL;
    lantern.editor_font_size = MAX_EDITOR_FONT_SIZE;

    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Scroll { lines: -4 }),
    ));

    assert_eq!(lantern.editor_font_size, MAX_EDITOR_FONT_SIZE);
}

#[test]
fn editor_scroll_invalidates_the_editor_pane_once() {
    let (mut lantern, _) = boot();

    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Scroll { lines: 4 }),
    ));

    assert!(lantern.editor_redraw_epoch);

    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Scroll { lines: 0 }),
    ));

    assert!(lantern.editor_redraw_epoch);
}

#[test]
fn selected_parent_creates_and_opens_a_new_project() {
    let parent = tempfile::tempdir().expect("temporary directory");
    let (mut lantern, _) = boot();
    lantern.new_project_name = "New Novel".to_owned();

    drop(update(
        &mut lantern,
        Message::ProjectParentPicked(Some(parent.path().to_owned())),
    ));

    assert_eq!(workspace_names(&lantern), WORKSPACE_DIRECTORIES);
    assert!(lantern.project_error.is_none());
    let project = lantern.project.expect("created project should be open");
    assert!(project.root().is_dir());
    assert_eq!(project.display_name(), "New Novel");
}

#[test]
fn opening_a_project_shows_the_workspace_directories_and_nothing_else() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(directory.path().join("chapters")).expect("chapters directory");
    std::fs::create_dir(directory.path().join("old drafts")).expect("other directory");
    std::fs::write(directory.path().join("notes.md"), "notes").expect("notes file");
    let (mut lantern, _) = boot();

    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    let rows = lantern.explorer.visible_rows();
    assert_eq!(
        rows.iter().map(|row| row.entry.name()).collect::<Vec<_>>(),
        WORKSPACE_DIRECTORIES
    );
    assert!(rows.iter().all(|row| row.depth == 0));
    // Left where the author put them, out of sight rather than removed.
    assert!(directory.path().join("old drafts").is_dir());
    assert!(directory.path().join("notes.md").is_file());
}

#[test]
fn opening_a_project_creates_the_workspace_directories_it_is_missing() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (mut lantern, _) = boot();

    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    for name in WORKSPACE_DIRECTORIES {
        assert!(directory.path().join(name).is_dir(), "{name} should exist");
    }
    assert_eq!(workspace_names(&lantern), WORKSPACE_DIRECTORIES);
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_file_standing_where_a_workspace_directory_belongs_stops_the_project_opening() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("drawer"), "not a directory").expect("blocking file");
    let (mut lantern, _) = boot();

    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    assert!(lantern.project.is_none());
    assert!(lantern.project_error.is_some());
}

#[test]
fn expanding_a_directory_loads_its_children() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(directory.path().join("chapters")).expect("chapters directory");
    std::fs::write(directory.path().join("chapters").join("one.md"), "one").expect("chapter file");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    let rows = lantern.explorer.visible_rows();
    assert_eq!(rows.len(), WORKSPACE_DIRECTORIES.len() + 1);
    assert_eq!(rows[0].entry.name(), "chapters");
    assert!(rows[0].expanded);
    assert_eq!(rows[1].entry.name(), "one.md");
    assert_eq!(rows[1].depth, 1);
}

#[test]
fn clicking_a_document_loads_it_into_the_editor() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/chapter.md", "A beginning");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    drop(update(
        &mut lantern,
        Message::OpenDocument(PathBuf::from("chapters/chapter.md")),
    ));

    assert_eq!(lantern.editor.text(), "A beginning");
    assert_eq!(
        lantern.open_document_path(),
        Some(Path::new("chapters/chapter.md"))
    );
    assert!(lantern.project_error.is_none());
}

#[test]
fn an_unsupported_file_does_not_replace_the_open_document() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/chapter.txt", "Keep me");
    write_document(directory.path(), "chapters/cover.png", "not an image");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::OpenDocument(PathBuf::from("chapters/chapter.txt")),
    ));

    drop(update(
        &mut lantern,
        Message::OpenDocument(PathBuf::from("chapters/cover.png")),
    ));

    assert_eq!(lantern.editor.text(), "Keep me");
    assert_eq!(
        lantern.open_document_path(),
        Some(Path::new("chapters/chapter.txt"))
    );
    assert!(lantern.project_error.is_some());
}

#[test]
fn collapsing_a_directory_releases_its_cached_listing() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(directory.path().join("chapters")).expect("chapters directory");
    std::fs::write(directory.path().join("chapters").join("one.md"), "one").expect("chapter file");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));
    assert!(lantern.explorer.has_listing(Path::new("chapters")));

    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    assert!(!lantern.explorer.has_listing(Path::new("chapters")));
    assert_eq!(
        lantern.explorer.visible_rows().len(),
        WORKSPACE_DIRECTORIES.len()
    );
}

#[test]
fn collapsing_a_directory_releases_the_listings_nested_under_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let nested = directory.path().join("chapters").join("act-one");
    std::fs::create_dir_all(&nested).expect("nested directories");
    std::fs::write(nested.join("one.md"), "one").expect("chapter file");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters").join("act-one")),
    ));
    assert_eq!(
        lantern.explorer.visible_rows().len(),
        WORKSPACE_DIRECTORIES.len() + 2
    );

    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    assert!(!lantern.explorer.has_listing(Path::new("chapters")));
    assert!(
        !lantern
            .explorer
            .has_listing(&PathBuf::from("chapters").join("act-one"))
    );
}

#[test]
fn re_expanding_a_directory_restores_the_shape_it_was_collapsed_with() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let nested = directory.path().join("chapters").join("act-one");
    std::fs::create_dir_all(&nested).expect("nested directories");
    std::fs::write(nested.join("one.md"), "one").expect("chapter file");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters").join("act-one")),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    let rows = lantern.explorer.visible_rows();
    assert_eq!(
        rows.iter().map(|row| row.entry.name()).collect::<Vec<_>>(),
        vec!["chapters", "act-one", "one.md", "references", "drawer"]
    );
    assert_eq!(
        rows.iter().map(|row| row.depth).collect::<Vec<_>>(),
        vec![0, 1, 2, 0, 0]
    );
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_directory_that_disappeared_while_collapsed_does_not_stay_expanded() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let chapters = directory.path().join("chapters");
    std::fs::create_dir(&chapters).expect("chapters directory");
    std::fs::write(chapters.join("one.md"), "one").expect("chapter file");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));
    std::fs::remove_dir_all(&chapters).expect("remove chapters");

    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    assert!(!lantern.explorer.is_expanded(Path::new("chapters")));
    assert!(lantern.project_error.is_some());
}

/// Writes a document into a project, creating the directories above it.
fn write_document(directory: &Path, relative_path: &str, text: &str) -> PathBuf {
    let path = directory.join(relative_path);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("document directory");
    }

    std::fs::write(&path, text).expect("document file");

    path
}

/// Opens `directory` as a project with `relative_path` in the editor.
fn open_project_document(directory: &Path, relative_path: &str) -> Lantern {
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::OpenDocument(PathBuf::from(relative_path)),
    ));

    lantern
}

/// Types `text` into the editor one character at a time.
fn type_text(lantern: &mut Lantern, text: &str) {
    for character in text.chars() {
        drop(update(
            lantern,
            Message::Edit(text_editor::Action::Edit(text_editor::Edit::Insert(
                character,
            ))),
        ));
    }
}

#[test]
fn saving_writes_the_edited_text_over_the_document() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = write_document(directory.path(), "chapters/chapter.md", "One");
    let mut lantern = open_project_document(directory.path(), "chapters/chapter.md");
    type_text(&mut lantern, "!");

    drop(update(&mut lantern, Message::SaveDocument));

    assert_eq!(std::fs::read_to_string(&path).expect("read back"), "!One");
    assert!(!lantern.unsaved_edits);
    assert!(lantern.project_error.is_none());
}

#[test]
fn an_edit_leaves_the_document_unsaved_until_it_is_written() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/chapter.md", "One");
    let mut lantern = open_project_document(directory.path(), "chapters/chapter.md");
    assert!(!lantern.unsaved_edits);

    type_text(&mut lantern, "!");
    assert!(lantern.unsaved_edits);

    drop(update(&mut lantern, Message::SaveDocument));
    assert!(!lantern.unsaved_edits);
}

#[test]
fn moving_around_the_document_does_not_make_it_unsaved() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/chapter.md", "One");
    let mut lantern = open_project_document(directory.path(), "chapters/chapter.md");

    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::SelectAll),
    ));
    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Scroll { lines: 2 }),
    ));

    assert!(!lantern.unsaved_edits);
}

#[test]
fn saving_an_unchanged_document_leaves_the_file_alone() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = write_document(directory.path(), "chapters/chapter.md", "One");
    let mut lantern = open_project_document(directory.path(), "chapters/chapter.md");
    // Written behind Lantern's back: a save that has nothing to store must not
    // overwrite it with the buffer it already agrees with.
    std::fs::write(&path, "Written elsewhere").expect("rewrite document");

    drop(update(&mut lantern, Message::SaveDocument));

    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "Written elsewhere"
    );
}

#[test]
fn an_edit_that_is_undone_is_not_written_again() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = write_document(directory.path(), "chapters/chapter.md", "One");
    let mut lantern = open_project_document(directory.path(), "chapters/chapter.md");
    type_text(&mut lantern, "!");
    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Edit(text_editor::Edit::Backspace)),
    ));
    std::fs::write(&path, "Written elsewhere").expect("rewrite document");

    drop(update(&mut lantern, Message::SaveDocument));

    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "Written elsewhere"
    );
    assert!(!lantern.unsaved_edits);
}

#[test]
fn saving_keeps_the_line_endings_the_document_was_opened_with() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = write_document(directory.path(), "chapters/chapter.md", "One\r\nTwo\r\n");
    let mut lantern = open_project_document(directory.path(), "chapters/chapter.md");
    type_text(&mut lantern, "!");

    drop(update(&mut lantern, Message::SaveDocument));

    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "!One\r\nTwo\r\n"
    );
}

#[test]
fn opening_another_document_saves_the_one_being_left() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let first = write_document(directory.path(), "chapters/one.md", "One");
    write_document(directory.path(), "chapters/two.md", "Two");
    let mut lantern = open_project_document(directory.path(), "chapters/one.md");
    type_text(&mut lantern, "!");

    drop(update(
        &mut lantern,
        Message::OpenDocument(PathBuf::from("chapters/two.md")),
    ));

    assert_eq!(std::fs::read_to_string(&first).expect("read back"), "!One");
    assert_eq!(lantern.editor.text(), "Two");
    assert!(!lantern.unsaved_edits);
}

#[test]
fn opening_another_project_saves_the_document_being_left() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("chapter.md");
    std::fs::write(&path, "One").expect("document file");
    let other = tempfile::tempdir().expect("other temporary directory");
    let mut lantern = open_project_document(directory.path(), "chapter.md");
    type_text(&mut lantern, "!");

    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(other.path().to_owned())),
    ));

    assert_eq!(std::fs::read_to_string(&path).expect("read back"), "!One");
    assert!(lantern.open_document_path().is_none());
    assert!(!lantern.unsaved_edits);
}

#[test]
fn saving_without_an_open_document_does_nothing() {
    let (mut lantern, _) = boot();

    drop(update(&mut lantern, Message::SaveDocument));

    assert!(lantern.project_error.is_none());
}

#[test]
fn a_document_that_disappeared_reports_the_failed_save_and_stays_unsaved() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = write_document(directory.path(), "chapters/chapter.md", "One");
    let mut lantern = open_project_document(directory.path(), "chapters/chapter.md");
    type_text(&mut lantern, "!");
    std::fs::remove_file(&path).expect("remove document");

    drop(update(&mut lantern, Message::SaveDocument));

    assert!(lantern.project_error.is_some());
    assert!(lantern.unsaved_edits);
    assert_eq!(lantern.editor.text(), "!One");
}

#[test]
fn the_autosave_ticker_keeps_asking_for_saves_until_it_is_dropped() {
    let requests = save_requests(std::time::Duration::from_millis(20));
    let mut requests = iced::futures::executor::block_on_stream(Box::pin(requests));

    assert!(matches!(requests.next(), Some(Message::SaveDocument)));
    assert!(matches!(requests.next(), Some(Message::SaveDocument)));
}
