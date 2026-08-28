use lantern_core::{ProjectEntry, ProjectName};
use lantern_store::{FsProjectStore, ProjectStore, StoreError};
use std::fs;
use std::path::Path;

#[test]
fn opens_an_existing_directory() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project = FsProjectStore
        .open_project(directory.path())
        .expect("project should open");

    assert_eq!(project.root(), directory.path().canonicalize().unwrap());
}

#[test]
fn creates_one_new_project_directory() {
    let parent = tempfile::tempdir().expect("temporary directory");
    let name = ProjectName::new("My Novel").expect("valid project name");
    let project = FsProjectStore
        .create_project(parent.path(), &name)
        .expect("project should be created");

    assert!(project.root().is_dir());
    assert_eq!(project.display_name(), "My Novel");
}

#[test]
fn lists_directories_first_and_hides_root_metadata() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(directory.path().join("chapters")).expect("chapters directory");
    fs::create_dir(directory.path().join(".lantern")).expect("metadata directory");
    fs::write(directory.path().join("notes.md"), "notes").expect("notes file");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    let entries = store
        .list_directory(&project, Path::new(""))
        .expect("list project root");

    assert_eq!(
        entries.iter().map(ProjectEntry::name).collect::<Vec<_>>(),
        vec!["chapters", "notes.md"]
    );
    assert!(entries[0].is_directory());
}

#[test]
fn rejects_paths_that_escape_the_project() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    assert!(matches!(
        store.list_directory(&project, Path::new("..")),
        Err(StoreError::UnsafeProjectPath(_))
    ));
}

#[test]
fn reads_a_utf8_document_by_project_relative_path() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::write(directory.path().join("chapter.md"), "Once upon a time").expect("document file");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    let document = store
        .read_document(&project, Path::new("chapter.md"))
        .expect("read document");

    assert_eq!(document.relative_path(), Path::new("chapter.md"));
    assert_eq!(document.content(), "Once upon a time");
}

#[test]
fn refuses_to_read_lantern_internal_state() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(directory.path().join(".lantern")).expect("metadata directory");
    fs::write(
        directory.path().join(".lantern").join("state.txt"),
        "private",
    )
    .expect("metadata file");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    assert!(matches!(
        store.read_document(&project, Path::new(".lantern/state.txt")),
        Err(StoreError::UnsafeProjectPath(_))
    ));
}
