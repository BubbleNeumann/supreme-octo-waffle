use super::*;

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

    let project = lantern.project.expect("created project should be open");
    assert!(project.root().is_dir());
    assert_eq!(project.display_name(), "New Novel");
    assert!(lantern.project_error.is_none());
    assert!(lantern.project_tree.is_empty());
}

#[test]
fn opening_a_project_loads_only_its_root_entries() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(directory.path().join("chapters")).expect("chapters directory");
    std::fs::write(directory.path().join("notes.md"), "notes").expect("notes file");
    std::fs::write(directory.path().join("chapters").join("one.md"), "one").expect("chapter file");
    let (mut lantern, _) = boot();

    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    assert_eq!(
        lantern
            .project_tree
            .iter()
            .map(|row| row.entry.name())
            .collect::<Vec<_>>(),
        vec!["chapters", "notes.md"]
    );
    assert!(lantern.project_tree.iter().all(|row| row.depth == 0));
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

    assert_eq!(lantern.project_tree.len(), 2);
    assert!(lantern.project_tree[0].expanded);
    assert_eq!(lantern.project_tree[1].entry.name(), "one.md");
    assert_eq!(lantern.project_tree[1].depth, 1);
}

#[test]
fn clicking_a_document_loads_it_into_the_editor() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("chapter.md"), "A beginning").expect("document file");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));

    drop(update(
        &mut lantern,
        Message::OpenDocument(PathBuf::from("chapter.md")),
    ));

    assert_eq!(lantern.editor.text(), "A beginning");
    assert_eq!(
        lantern.open_document.as_deref(),
        Some(Path::new("chapter.md"))
    );
    assert!(lantern.project_error.is_none());
}

#[test]
fn an_unsupported_file_does_not_replace_the_open_document() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("chapter.txt"), "Keep me").expect("document file");
    std::fs::write(directory.path().join("cover.png"), "not an image").expect("other file");
    let (mut lantern, _) = boot();
    drop(update(
        &mut lantern,
        Message::OpenProjectFolderPicked(Some(directory.path().to_owned())),
    ));
    drop(update(
        &mut lantern,
        Message::OpenDocument(PathBuf::from("chapter.txt")),
    ));

    drop(update(
        &mut lantern,
        Message::OpenDocument(PathBuf::from("cover.png")),
    ));

    assert_eq!(lantern.editor.text(), "Keep me");
    assert_eq!(
        lantern.open_document.as_deref(),
        Some(Path::new("chapter.txt"))
    );
    assert!(lantern.project_error.is_some());
}
