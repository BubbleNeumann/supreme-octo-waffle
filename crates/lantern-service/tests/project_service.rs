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
