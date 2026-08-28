use lantern_service::{ProjectService, ProjectServiceError};
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
