use lantern_service::{ProjectService, ProjectServiceError, WORKSPACE_DIRECTORIES};
use std::path::Path;

#[test]
fn creates_then_opens_a_filesystem_project() {
    let parent = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let created = service
        .create_project(parent.path(), "Novel")
        .expect("project should be created");
    let opened = service
        .open_project(created.root())
        .expect("project should reopen");

    assert_eq!(opened, created);
}

#[test]
fn opens_supported_documents_and_rejects_other_files() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("chapter.MD"), "Chapter one").expect("document file");
    std::fs::write(directory.path().join("cover.png"), "not an image").expect("other file");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");

    let document = service
        .open_document(&project, Path::new("chapter.MD"))
        .expect("document should open");

    assert_eq!(document.content(), "Chapter one");
    assert!(matches!(
        service.open_document(&project, Path::new("cover.png")),
        Err(ProjectServiceError::UnsupportedDocument(_))
    ));
}

#[test]
fn saving_an_unedited_document_reproduces_the_original_bytes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("chapter.md");
    let original = "\u{feff}One\r\nTwo\r\n";
    std::fs::write(&path, original).expect("document file");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    let mut document = service
        .open_document(&project, Path::new("chapter.md"))
        .expect("document should open");
    let unedited = document.content().to_owned();

    service
        .save_document(&project, &mut document, &unedited)
        .expect("document should save");

    assert_eq!(std::fs::read_to_string(&path).expect("read back"), original);
}

#[test]
fn saving_edited_text_keeps_the_original_conventions() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("chapter.md");
    std::fs::write(&path, "One\r\nTwo\r\n").expect("document file");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    let mut document = service
        .open_document(&project, Path::new("chapter.md"))
        .expect("document should open");

    service
        .save_document(&project, &mut document, "One\nTwo\nThree\n")
        .expect("document should save");

    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "One\r\nTwo\r\nThree\r\n"
    );
}

#[test]
fn a_saved_document_stops_reporting_the_text_it_kept_as_a_change() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("chapter.md"), "One\n").expect("document file");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    let mut document = service
        .open_document(&project, Path::new("chapter.md"))
        .expect("document should open");
    assert!(document.differs_from("One\nTwo\n"));

    service
        .save_document(&project, &mut document, "One\nTwo\n")
        .expect("document should save");

    assert_eq!(document.content(), "One\nTwo\n");
    assert!(!document.differs_from("One\nTwo\n"));
}

#[test]
fn a_failed_save_leaves_the_document_describing_the_file_on_disk() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("chapter.md");
    std::fs::write(&path, "One\n").expect("document file");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    let mut document = service
        .open_document(&project, Path::new("chapter.md"))
        .expect("document should open");
    std::fs::remove_file(&path).expect("remove document");

    assert!(
        service
            .save_document(&project, &mut document, "One\nTwo\n")
            .is_err()
    );
    assert_eq!(document.content(), "One\n");
}

#[test]
fn opening_a_project_creates_the_workspace_directories() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();

    let project = service
        .open_project(directory.path())
        .expect("project should open");

    for name in WORKSPACE_DIRECTORIES {
        assert!(project.root().join(name).is_dir(), "{name} should exist");
    }
}

#[test]
fn creating_a_project_creates_the_workspace_directories() {
    let parent = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();

    let project = service
        .create_project(parent.path(), "Novel")
        .expect("project should be created");

    for name in WORKSPACE_DIRECTORIES {
        assert!(project.root().join(name).is_dir(), "{name} should exist");
    }
}

#[test]
fn opening_a_project_leaves_the_workspace_directories_it_finds_alone() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let chapter = directory.path().join("chapters").join("one.md");
    std::fs::create_dir(directory.path().join("chapters")).expect("chapters directory");
    std::fs::write(&chapter, "one").expect("chapter file");
    let service = ProjectService::filesystem();

    service
        .open_project(directory.path())
        .expect("project should open");

    assert_eq!(std::fs::read_to_string(&chapter).expect("read back"), "one");
}

#[test]
fn lists_only_the_workspace_directories_at_the_project_root() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(directory.path().join("old drafts")).expect("other directory");
    std::fs::write(directory.path().join("notes.md"), "notes").expect("other file");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");

    let entries = service
        .list_directory(&project, Path::new(""))
        .expect("root should list");

    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        WORKSPACE_DIRECTORIES
    );
    // Hidden from the explorer, but still where the author left them.
    assert!(directory.path().join("old drafts").is_dir());
    assert!(directory.path().join("notes.md").is_file());
}

#[test]
fn lists_a_workspace_directory_in_full() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let chapters = directory.path().join("chapters");
    std::fs::create_dir(&chapters).expect("chapters directory");
    std::fs::create_dir(chapters.join("act-one")).expect("nested directory");
    std::fs::write(chapters.join("one.md"), "one").expect("chapter file");
    std::fs::write(chapters.join("cover.png"), "cover").expect("other file");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");

    let entries = service
        .list_directory(&project, Path::new("chapters"))
        .expect("chapters should list");

    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        vec!["act-one", "cover.png", "one.md"]
    );
}

#[test]
fn a_directory_named_in_another_case_counts_as_the_workspace_directory() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(directory.path().join("Chapters")).expect("chapters directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");

    let entries = service
        .list_directory(&project, Path::new(""))
        .expect("root should list");

    assert_eq!(entries.len(), WORKSPACE_DIRECTORIES.len());
    assert_eq!(entries[0].name(), "Chapters");
}

#[test]
fn refuses_to_open_a_project_that_cannot_hold_the_workspace() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("drawer"), "not a directory").expect("blocking file");
    let service = ProjectService::filesystem();

    assert!(service.open_project(directory.path()).is_err());
}

#[test]
fn creates_a_document_and_opens_it_empty() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");

    let document = service
        .create_document(&project, Path::new("chapters"), "Chapter One")
        .expect("document should be created");

    assert_eq!(
        document.relative_path(),
        Path::new("chapters").join("Chapter One.md")
    );
    assert_eq!(document.content(), "");
    assert!(
        directory
            .path()
            .join("chapters")
            .join("Chapter One.md")
            .is_file()
    );
}

#[test]
fn a_created_document_opens_again_through_the_service() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    let created = service
        .create_document(&project, Path::new("references"), "sources.txt")
        .expect("document should be created");

    let opened = service
        .open_document(&project, created.relative_path())
        .expect("document should open");

    assert_eq!(opened, created);
}

#[test]
fn refuses_to_create_a_document_over_one_already_written() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    let document_path = directory.path().join("chapters").join("one.md");
    std::fs::write(&document_path, "Chapter one").expect("document file");

    let result = service.create_document(&project, Path::new("chapters"), "one.md");

    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(&document_path).expect("read"),
        "Chapter one"
    );
}

#[test]
fn refuses_a_document_name_that_is_not_a_single_file_name() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");

    assert!(matches!(
        service.create_document(&project, Path::new("chapters"), "../one"),
        Err(ProjectServiceError::InvalidDocumentName(_))
    ));
    assert!(matches!(
        service.create_document(&project, Path::new("chapters"), "   "),
        Err(ProjectServiceError::InvalidDocumentName(_))
    ));
}

#[test]
fn a_moved_document_opens_at_the_path_it_moved_to() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    std::fs::write(
        directory.path().join("chapters").join("one.md"),
        "Chapter one",
    )
    .expect("document file");

    let moved = service
        .move_document(&project, Path::new("chapters/one.md"), Path::new("drawer"))
        .expect("document should move");

    let document = service
        .open_document(&project, &moved)
        .expect("document should open");
    assert_eq!(document.content(), "Chapter one");
    assert_eq!(document.relative_path(), Path::new("drawer").join("one.md"));
}

#[test]
fn refuses_to_move_a_document_that_is_not_there() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");

    assert!(
        service
            .move_document(&project, Path::new("chapters/gone.md"), Path::new("drawer"))
            .is_err()
    );
}

#[test]
fn a_placed_document_is_listed_where_it_was_put() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    for name in ["one.md", "two.md", "three.md"] {
        std::fs::write(directory.path().join("chapters").join(name), name).expect("document file");
    }

    // "three.md" sorts first by name; put it back where it belongs.
    service
        .place_document(
            &project,
            Path::new("chapters/three.md"),
            Path::new("chapters"),
            None,
        )
        .expect("document should be placed");

    let entries = service
        .list_directory(&project, Path::new("chapters"))
        .expect("chapters should list");
    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        vec!["one.md", "two.md", "three.md"]
    );
}

#[test]
fn a_document_placed_before_another_is_listed_before_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    for name in ["one.md", "two.md"] {
        std::fs::write(directory.path().join("chapters").join(name), name).expect("document file");
    }

    service
        .place_document(
            &project,
            Path::new("chapters/two.md"),
            Path::new("chapters"),
            Some("one.md"),
        )
        .expect("document should be placed");

    let entries = service
        .list_directory(&project, Path::new("chapters"))
        .expect("chapters should list");
    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        vec!["two.md", "one.md"]
    );
}

#[test]
fn placing_a_document_from_another_directory_moves_it_as_well() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    std::fs::write(directory.path().join("chapters").join("one.md"), "one").expect("document file");
    std::fs::write(directory.path().join("drawer").join("cut.md"), "cut").expect("document file");

    let placed = service
        .place_document(
            &project,
            Path::new("drawer/cut.md"),
            Path::new("chapters"),
            Some("one.md"),
        )
        .expect("document should be placed");

    assert_eq!(placed, Path::new("chapters").join("cut.md"));
    assert!(!directory.path().join("drawer").join("cut.md").exists());
    let entries = service
        .list_directory(&project, Path::new("chapters"))
        .expect("chapters should list");
    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        vec!["cut.md", "one.md"]
    );
}

#[test]
fn an_order_holds_across_reopening_the_project() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    for name in ["one.md", "two.md"] {
        std::fs::write(directory.path().join("chapters").join(name), name).expect("document file");
    }
    service
        .place_document(
            &project,
            Path::new("chapters/two.md"),
            Path::new("chapters"),
            Some("one.md"),
        )
        .expect("document should be placed");

    let reopened = service
        .open_project(directory.path())
        .expect("project should reopen");

    let entries = service
        .list_directory(&reopened, Path::new("chapters"))
        .expect("chapters should list");
    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        vec!["two.md", "one.md"]
    );
}

#[test]
fn a_document_added_outside_lantern_is_listed_after_the_ordered_ones() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    for name in ["one.md", "two.md"] {
        std::fs::write(directory.path().join("chapters").join(name), name).expect("document file");
    }
    service
        .place_document(
            &project,
            Path::new("chapters/two.md"),
            Path::new("chapters"),
            Some("one.md"),
        )
        .expect("document should be placed");

    std::fs::write(
        directory.path().join("chapters").join("a-new-one.md"),
        "new",
    )
    .expect("document written by another program");

    let entries = service
        .list_directory(&project, Path::new("chapters"))
        .expect("chapters should list");
    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        vec!["two.md", "one.md", "a-new-one.md"]
    );
}

#[test]
fn deleting_the_order_file_leaves_the_documents_listed_by_name() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");
    for name in ["one.md", "two.md"] {
        std::fs::write(directory.path().join("chapters").join(name), name).expect("document file");
    }
    service
        .place_document(
            &project,
            Path::new("chapters/two.md"),
            Path::new("chapters"),
            Some("one.md"),
        )
        .expect("document should be placed");

    std::fs::remove_dir_all(directory.path().join(".lantern")).expect("remove lantern state");

    let entries = service
        .list_directory(&project, Path::new("chapters"))
        .expect("chapters should list");
    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        vec!["one.md", "two.md"]
    );
}

#[test]
fn the_project_root_keeps_the_workspace_order() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory.path())
        .expect("project should open");

    let entries = service
        .list_directory(&project, Path::new(""))
        .expect("root should list");

    assert_eq!(
        entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        WORKSPACE_DIRECTORIES
    );
}
