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

#[test]
fn a_new_document_is_created_in_the_chapters_directory_and_opened() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    drop(update(&mut lantern, Message::BeginCreateDocument));
    drop(update(
        &mut lantern,
        Message::NewDocumentNameChanged("Chapter One".to_owned()),
    ));
    drop(update(&mut lantern, Message::CreateDocument));

    let relative_path = PathBuf::from("chapters").join("Chapter One.md");
    assert!(directory.path().join(&relative_path).is_file());
    assert_eq!(lantern.open_document_path(), Some(relative_path.as_path()));
    assert_eq!(lantern.editor.text(), "");
    assert!(!lantern.creating_document);
    assert!(lantern.new_document_name.is_empty());
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_new_document_is_shown_in_the_directory_it_was_created_in() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::NewDocumentNameChanged("Chapter One".to_owned()),
    ));

    drop(update(&mut lantern, Message::CreateDocument));

    let rows = lantern.explorer.visible_rows();
    assert_eq!(
        rows.iter().map(|row| row.entry.name()).collect::<Vec<_>>(),
        vec!["chapters", "Chapter One.md", "references", "drawer"]
    );
    assert!(lantern.explorer.is_expanded(Path::new("chapters")));
}

#[test]
fn a_new_document_is_created_beside_the_open_one() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "references/sources.md", "Sources");
    let mut lantern = open_project_document(directory.path(), "references/sources.md");
    drop(update(
        &mut lantern,
        Message::NewDocumentNameChanged("Timeline".to_owned()),
    ));

    drop(update(&mut lantern, Message::CreateDocument));

    let relative_path = PathBuf::from("references").join("Timeline.md");
    assert!(directory.path().join(&relative_path).is_file());
    assert_eq!(lantern.open_document_path(), Some(relative_path.as_path()));
}

#[test]
fn creating_a_document_keeps_the_edits_in_the_one_being_left() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let document_path = write_document(directory.path(), "chapters/one.md", "A beginning");
    let mut lantern = open_project_document(directory.path(), "chapters/one.md");
    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Edit(text_editor::Edit::Insert('!'))),
    ));
    drop(update(
        &mut lantern,
        Message::NewDocumentNameChanged("two".to_owned()),
    ));

    drop(update(&mut lantern, Message::CreateDocument));

    assert_eq!(
        std::fs::read_to_string(&document_path).expect("read"),
        "!A beginning"
    );
    assert_eq!(lantern.editor.text(), "");
    assert!(!lantern.unsaved_edits);
}

#[test]
fn a_name_already_in_use_leaves_the_document_on_disk_alone() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let document_path = write_document(directory.path(), "chapters/one.md", "A beginning");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::NewDocumentNameChanged("one.md".to_owned()),
    ));

    drop(update(&mut lantern, Message::CreateDocument));

    assert_eq!(
        std::fs::read_to_string(&document_path).expect("read"),
        "A beginning"
    );
    assert!(lantern.open_document.is_none());
    assert!(lantern.project_error.is_some());
    // The name stays in the field, because it is the name that needs changing.
    assert_eq!(lantern.new_document_name, "one.md");
}

#[test]
fn no_document_is_created_without_a_project_to_create_it_in() {
    let (mut lantern, _) = boot();

    drop(update(&mut lantern, Message::BeginCreateDocument));
    drop(update(
        &mut lantern,
        Message::NewDocumentNameChanged("Chapter One".to_owned()),
    ));
    drop(update(&mut lantern, Message::CreateDocument));

    assert!(lantern.project.is_none());
    assert!(lantern.open_document.is_none());
    assert!(lantern.project_error.is_none());
}

#[test]
fn dragging_a_document_onto_a_directory_moves_it_there() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "Chapter one");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    drag_document(&mut lantern, "chapters/one.md", "drawer");

    assert!(!directory.path().join("chapters").join("one.md").exists());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("drawer").join("one.md")).expect("read"),
        "Chapter one"
    );
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_moved_document_is_drawn_in_the_directory_it_was_dropped_into() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "Chapter one");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    drag_document(&mut lantern, "chapters/one.md", "drawer");

    let rows = lantern.explorer.visible_rows();
    assert_eq!(
        rows.iter().map(|row| row.entry.name()).collect::<Vec<_>>(),
        vec!["chapters", "references", "drawer", "one.md"]
    );
    assert!(lantern.explorer.is_expanded(Path::new("drawer")));
}

#[test]
fn the_open_document_keeps_its_unsaved_text_when_it_is_moved() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "A beginning");
    let mut lantern = open_project_document(directory.path(), "chapters/one.md");
    drop(update(
        &mut lantern,
        Message::Edit(text_editor::Action::Edit(text_editor::Edit::Insert('!'))),
    ));

    drag_document(&mut lantern, "chapters/one.md", "drawer");

    let moved_path = PathBuf::from("drawer").join("one.md");
    assert_eq!(lantern.open_document_path(), Some(moved_path.as_path()));
    assert_eq!(lantern.editor.text(), "!A beginning");
    assert!(lantern.unsaved_edits);

    // The next save writes where the document is now, not where it was.
    drop(update(&mut lantern, Message::SaveDocument));
    assert_eq!(
        std::fs::read_to_string(directory.path().join(&moved_path)).expect("read"),
        "!A beginning"
    );
}

#[test]
fn clicking_a_document_is_not_a_drag() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "Chapter one");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    // Pressed and let go over the one row, which is where a click happens.
    drop(update(
        &mut lantern,
        Message::EntryHovered(Some(HoveredEntry::Document {
            relative_path: PathBuf::from("chapters/one.md"),
            place: DropPlace::After,
        })),
    ));
    drop(update(&mut lantern, Message::PointerPressed));
    drop(update(&mut lantern, Message::PointerReleased));

    assert!(directory.path().join("chapters").join("one.md").is_file());
    assert!(lantern.dragged_document.is_none());
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_document_let_go_over_the_directory_it_is_already_in_does_not_move() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "Chapter one");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    drag_document(&mut lantern, "chapters/one.md", "chapters");

    assert!(directory.path().join("chapters").join("one.md").is_file());
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_document_let_go_outside_the_explorer_does_not_move() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "Chapter one");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::EntryHovered(Some(HoveredEntry::Document {
            relative_path: PathBuf::from("chapters/one.md"),
            place: DropPlace::After,
        })),
    ));
    drop(update(&mut lantern, Message::PointerPressed));

    // The pointer leaves the tree, naming no row, and the button is let go.
    drop(update(&mut lantern, Message::EntryHovered(None)));
    drop(update(&mut lantern, Message::PointerReleased));

    assert!(directory.path().join("chapters").join("one.md").is_file());
    assert!(lantern.dragged_document.is_none());
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_directory_is_not_dragged_by_its_row() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "Chapter one");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    drop(update(
        &mut lantern,
        Message::EntryHovered(Some(HoveredEntry::Directory(PathBuf::from("chapters")))),
    ));
    drop(update(&mut lantern, Message::PointerPressed));
    assert!(lantern.dragged_document.is_none());

    drop(update(
        &mut lantern,
        Message::EntryHovered(Some(HoveredEntry::Directory(PathBuf::from("drawer")))),
    ));
    drop(update(&mut lantern, Message::PointerReleased));

    assert!(directory.path().join("chapters").is_dir());
    assert!(!directory.path().join("drawer").join("chapters").exists());
}

#[test]
fn a_name_already_taken_where_it_was_dropped_leaves_both_documents_alone() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "Chapter one");
    write_document(directory.path(), "drawer/one.md", "An older one");
    let mut lantern = open_project_document(directory.path(), "chapters/one.md");

    drag_document(&mut lantern, "chapters/one.md", "drawer");

    assert_eq!(
        std::fs::read_to_string(directory.path().join("chapters").join("one.md")).expect("read"),
        "Chapter one"
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("drawer").join("one.md")).expect("read"),
        "An older one"
    );
    assert_eq!(
        lantern.open_document_path(),
        Some(Path::new("chapters/one.md"))
    );
    assert!(lantern.project_error.is_some());
}

/// Presses over a document row, moves onto a directory row, and lets go.
fn drag_document(lantern: &mut Lantern, document: &str, directory: &str) {
    drop(update(
        lantern,
        Message::EntryHovered(Some(HoveredEntry::Document {
            relative_path: PathBuf::from(document),
            place: DropPlace::After,
        })),
    ));
    drop(update(lantern, Message::PointerPressed));
    drop(update(
        lantern,
        Message::EntryHovered(Some(HoveredEntry::Directory(PathBuf::from(directory)))),
    ));
    drop(update(lantern, Message::PointerReleased));
}

#[test]
fn dragging_a_document_above_another_reorders_them() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "One");
    write_document(directory.path(), "chapters/two.md", "Two");
    let mut lantern = open_chapters(directory.path());
    assert_eq!(chapter_names(&lantern), vec!["one.md", "two.md"]);

    drag_document_against(
        &mut lantern,
        "chapters/two.md",
        "chapters/one.md",
        DropPlace::Before,
    );

    assert_eq!(chapter_names(&lantern), vec!["two.md", "one.md"]);
    assert!(lantern.project_error.is_none());
}

#[test]
fn dragging_a_document_below_another_puts_it_after_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "One");
    write_document(directory.path(), "chapters/two.md", "Two");
    write_document(directory.path(), "chapters/three.md", "Three");
    let mut lantern = open_chapters(directory.path());
    assert_eq!(
        chapter_names(&lantern),
        vec!["one.md", "three.md", "two.md"]
    );

    // Let go over the lower part of "one.md", which means after it.
    drag_document_against(
        &mut lantern,
        "chapters/two.md",
        "chapters/one.md",
        DropPlace::After,
    );

    assert_eq!(
        chapter_names(&lantern),
        vec!["one.md", "two.md", "three.md"]
    );
}

#[test]
fn an_order_a_drag_gave_outlasts_the_session_that_gave_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "One");
    write_document(directory.path(), "chapters/two.md", "Two");
    let mut lantern = open_chapters(directory.path());
    drag_document_against(
        &mut lantern,
        "chapters/two.md",
        "chapters/one.md",
        DropPlace::Before,
    );

    let reopened = open_chapters(directory.path());

    assert_eq!(chapter_names(&reopened), vec!["two.md", "one.md"]);
}

#[test]
fn a_document_dragged_onto_itself_changes_nothing() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "One");
    write_document(directory.path(), "chapters/two.md", "Two");
    let mut lantern = open_chapters(directory.path());

    drag_document_against(
        &mut lantern,
        "chapters/one.md",
        "chapters/one.md",
        DropPlace::Before,
    );

    assert_eq!(chapter_names(&lantern), vec!["one.md", "two.md"]);
    assert!(!directory.path().join(".lantern").exists());
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_document_dragged_against_one_elsewhere_moves_and_takes_that_place() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "One");
    write_document(directory.path(), "chapters/two.md", "Two");
    write_document(directory.path(), "drawer/cut.md", "Cut");
    let mut lantern = open_chapters(directory.path());
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("drawer")),
    ));

    drag_document_against(
        &mut lantern,
        "drawer/cut.md",
        "chapters/two.md",
        DropPlace::Before,
    );

    assert!(!directory.path().join("drawer").join("cut.md").exists());
    assert!(directory.path().join("chapters").join("cut.md").is_file());
    assert_eq!(chapter_names(&lantern), vec!["one.md", "cut.md", "two.md"]);
}

#[test]
fn dropping_a_document_onto_a_folder_records_no_order() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "One");
    let mut lantern = open_chapters(directory.path());

    drag_document(&mut lantern, "chapters/one.md", "drawer");

    assert!(directory.path().join("drawer").join("one.md").is_file());
    // Moving a document is not ordering one, and a project that has never been
    // ordered keeps no state saying so.
    assert!(!directory.path().join(".lantern").exists());
}

/// Opens `directory` as a project with its chapters expanded.
fn open_chapters(directory: &Path) -> Lantern {
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));

    lantern
}

/// Returns the names drawn under `chapters`, top to bottom.
fn chapter_names(lantern: &Lantern) -> Vec<&str> {
    lantern
        .explorer
        .listing(Path::new("chapters"))
        .expect("chapters should be listed")
        .iter()
        .map(|entry| entry.name())
        .collect()
}

/// Presses over a document row and lets go against another document's row.
///
/// `place` is what where the pointer ended up in that row asked for.
fn drag_document_against(lantern: &mut Lantern, document: &str, against: &str, place: DropPlace) {
    drop(update(
        lantern,
        Message::EntryHovered(Some(HoveredEntry::Document {
            relative_path: PathBuf::from(document),
            place: DropPlace::After,
        })),
    ));
    drop(update(lantern, Message::PointerPressed));
    drop(update(
        lantern,
        Message::EntryHovered(Some(HoveredEntry::Document {
            relative_path: PathBuf::from(against),
            place,
        })),
    ));
    drop(update(lantern, Message::PointerReleased));
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

#[test]
fn the_editor_takes_no_writing_until_a_project_is_open() {
    let (lantern, _) = boot();

    assert!(!lantern.accepts_writing());
}

#[test]
fn opening_a_project_lets_the_editor_be_written_in() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (mut lantern, _) = boot();

    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    assert!(lantern.accepts_writing());
}

#[test]
fn a_project_that_fails_to_open_leaves_the_editor_inert() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("drawer"), "not a directory").expect("blocking file");
    let (mut lantern, _) = boot();

    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    assert!(lantern.project_error.is_some());
    assert!(!lantern.accepts_writing());
}

#[test]
fn the_window_carries_the_applications_icon() {
    let icon = window_icon().expect("the bundled icon should decode");

    let (pixels, size) = icon.into_raw();

    // Square, so that a system scaling it for a small slot does not stretch
    // it, and four bytes a pixel because that is what Iced was handed.
    assert_eq!(size.width, size.height);
    assert_eq!(pixels.len(), (size.width * size.height * 4) as usize);
}

#[test]
fn the_window_icon_is_large_enough_to_scale_down_to_every_slot() {
    let icon = window_icon().expect("the bundled icon should decode");

    // The largest slot a taskbar asks for is 64 pixels at 200% scaling, and
    // scaling a mark up is what makes it look soft.
    let (_, size) = icon.into_raw();

    assert!(size.width >= 64, "the icon is {} pixels wide", size.width);
}

/// Presses over a document row and lets go over the middle of a chapter's.
fn drag_document_under(lantern: &mut Lantern, document: &str, chapter: &str) {
    drag_document_against(lantern, document, chapter, DropPlace::Under);
}

/// Returns the rows drawn for the tree, as their depth and drawn name.
fn drawn_rows(lantern: &Lantern) -> Vec<(usize, String)> {
    lantern
        .explorer
        .visible_rows()
        .into_iter()
        .map(|row| {
            (
                row.depth,
                crate::ui::sidebar::row_title(row.entry.name(), row.chapter_number).into_owned(),
            )
        })
        .collect()
}

#[test]
fn a_document_let_go_over_a_chapter_becomes_a_scene_under_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/Arrival.md", "");
    write_document(directory.path(), "drawer/The station.md", "Rain.");
    let mut lantern = open_chapters(directory.path());
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("drawer")),
    ));

    drag_document_under(&mut lantern, "drawer/The station.md", "chapters/Arrival.md");

    assert!(
        directory
            .path()
            .join("chapters")
            .join("Arrival")
            .join("The station.md")
            .is_file()
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("chapters").join("Arrival.md"))
            .expect("chapter should be read"),
        "Rain."
    );
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_chapter_that_has_scenes_is_drawn_as_the_folder_holding_them() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/Arrival.md", "Rain.");
    write_document(directory.path(), "chapters/Arrival/The station.md", "Rain.");
    let mut lantern = open_chapters(directory.path());

    assert_eq!(
        drawn_rows(&lantern),
        vec![
            (0, "chapters".to_owned()),
            (1, "Chapter 1. Arrival".to_owned()),
            (0, "references".to_owned()),
            (0, "drawer".to_owned()),
        ]
    );

    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters/Arrival")),
    ));

    assert_eq!(
        drawn_rows(&lantern),
        vec![
            (0, "chapters".to_owned()),
            (1, "Chapter 1. Arrival".to_owned()),
            (2, "The station".to_owned()),
            (0, "references".to_owned()),
            (0, "drawer".to_owned()),
        ]
    );
}

#[test]
fn a_chapter_carries_the_number_of_the_place_it_was_dragged_to() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "One");
    write_document(directory.path(), "chapters/two.md", "Two");
    let mut lantern = open_chapters(directory.path());
    assert_eq!(
        drawn_rows(&lantern)[1..3],
        [
            (1, "Chapter 1. one".to_owned()),
            (1, "Chapter 2. two".to_owned())
        ]
    );

    drag_document_against(
        &mut lantern,
        "chapters/two.md",
        "chapters/one.md",
        DropPlace::Before,
    );

    assert_eq!(
        drawn_rows(&lantern)[1..3],
        [
            (1, "Chapter 1. two".to_owned()),
            (1, "Chapter 2. one".to_owned())
        ]
    );
}

#[test]
fn a_scene_is_not_numbered_as_a_chapter() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/Arrival.md", "Rain.");
    write_document(directory.path(), "chapters/Arrival/The station.md", "Rain.");
    let mut lantern = open_chapters(directory.path());
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters/Arrival")),
    ));

    let numbers: Vec<Option<usize>> = lantern
        .explorer
        .visible_rows()
        .into_iter()
        .map(|row| row.chapter_number)
        .collect();

    assert_eq!(numbers, vec![None, Some(1), None, None, None]);
}

#[test]
fn a_chapter_gaining_its_first_scene_becomes_a_row_that_opens() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/Arrival.md", "");
    write_document(directory.path(), "drawer/The station.md", "Rain.");
    let mut lantern = open_chapters(directory.path());
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("drawer")),
    ));

    drag_document_under(&mut lantern, "drawer/The station.md", "chapters/Arrival.md");

    assert_eq!(
        drawn_rows(&lantern),
        vec![
            (0, "chapters".to_owned()),
            (1, "Chapter 1. Arrival".to_owned()),
            (2, "The station".to_owned()),
            (0, "references".to_owned()),
            (0, "drawer".to_owned()),
        ]
    );
}

#[test]
fn a_scene_let_go_over_the_chapter_it_is_already_under_does_not_move() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/Arrival.md", "Rain.");
    write_document(directory.path(), "chapters/Arrival/The station.md", "Rain.");
    let mut lantern = open_chapters(directory.path());

    drag_document_under(
        &mut lantern,
        "chapters/Arrival/The station.md",
        "chapters/Arrival.md",
    );

    assert!(
        directory
            .path()
            .join("chapters")
            .join("Arrival")
            .join("The station.md")
            .is_file()
    );
    assert!(lantern.project_error.is_none());
}

#[test]
fn a_document_let_go_over_the_middle_of_an_ordinary_row_is_ordered_after_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "drawer/one.md", "One");
    write_document(directory.path(), "drawer/two.md", "Two");
    write_document(directory.path(), "drawer/three.md", "Three");
    let mut lantern = open_chapters(directory.path());
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("drawer")),
    ));

    drag_document_against(
        &mut lantern,
        "drawer/two.md",
        "drawer/one.md",
        DropPlace::Under,
    );

    assert!(!directory.path().join("drawer").join("one").exists());
    assert_eq!(
        lantern
            .explorer
            .listing(Path::new("drawer"))
            .expect("drawer should be listed")
            .iter()
            .map(|entry| entry.name())
            .collect::<Vec<_>>(),
        vec!["one.md", "two.md", "three.md"]
    );
}

#[test]
fn editing_a_chapter_writes_the_scenes_it_is_written_in() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/Arrival.md", "Rain.");
    let scene = write_document(directory.path(), "chapters/Arrival/The station.md", "Rain.");
    let mut lantern = open_project_document(directory.path(), "chapters/Arrival.md");

    type_text(&mut lantern, "Cold ");
    drop(update(&mut lantern, Message::SaveDocument));

    assert_eq!(
        std::fs::read_to_string(&scene).expect("scene should be read"),
        "Cold Rain."
    );
}

#[test]
fn editing_a_scene_writes_the_chapter_it_is_under() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let chapter = write_document(directory.path(), "chapters/Arrival.md", "Rain.");
    write_document(directory.path(), "chapters/Arrival/The station.md", "Rain.");
    let mut lantern = open_project_document(directory.path(), "chapters/Arrival/The station.md");

    type_text(&mut lantern, "Cold ");
    drop(update(&mut lantern, Message::SaveDocument));

    assert_eq!(
        std::fs::read_to_string(&chapter).expect("chapter should be read"),
        "Cold Rain."
    );
}

#[test]
fn a_scene_dropped_under_the_open_chapter_is_read_into_the_editor() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/Arrival.md", "Rain.");
    write_document(directory.path(), "drawer/The house.md", "Gate.");
    let mut lantern = open_project_document(directory.path(), "chapters/Arrival.md");
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("drawer")),
    ));

    drag_document_under(&mut lantern, "drawer/The house.md", "chapters/Arrival.md");

    assert_eq!(lantern.editor.text(), "Rain.\n--\nGate.");
    assert!(!lantern.unsaved_edits);
}

#[test]
fn the_edits_in_an_open_chapter_survive_a_scene_being_dropped_under_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/Arrival.md", "Rain.");
    write_document(directory.path(), "drawer/The house.md", "Gate.");
    let mut lantern = open_project_document(directory.path(), "chapters/Arrival.md");
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("drawer")),
    ));
    type_text(&mut lantern, "Cold ");

    drag_document_under(&mut lantern, "drawer/The house.md", "chapters/Arrival.md");

    assert_eq!(lantern.editor.text(), "Cold Rain.\n--\nGate.");
    assert_eq!(
        std::fs::read_to_string(
            directory
                .path()
                .join("chapters")
                .join("Arrival")
                .join("Arrival.md")
        )
        .expect("scene should be read"),
        "Cold Rain."
    );
}

#[test]
fn an_open_chapter_being_reordered_keeps_the_text_it_has_not_saved() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_document(directory.path(), "chapters/one.md", "One");
    write_document(directory.path(), "chapters/two.md", "Two");
    let mut lantern = open_project_document(directory.path(), "chapters/two.md");
    drop(update(
        &mut lantern,
        Message::ToggleProjectDirectory(PathBuf::from("chapters")),
    ));
    type_text(&mut lantern, "!");

    drag_document_against(
        &mut lantern,
        "chapters/two.md",
        "chapters/one.md",
        DropPlace::Before,
    );

    assert_eq!(lantern.editor.text(), "!Two");
    assert!(lantern.unsaved_edits);
    assert_eq!(chapter_names(&lantern), vec!["two.md", "one.md"]);
}
