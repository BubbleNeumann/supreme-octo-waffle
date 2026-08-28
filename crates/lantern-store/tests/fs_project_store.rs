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

#[test]
fn refuses_to_create_a_project_over_an_existing_directory() {
    let parent = tempfile::tempdir().expect("temporary directory");
    let name = ProjectName::new("My Novel").expect("valid project name");
    let store = FsProjectStore;
    store
        .create_project(parent.path(), &name)
        .expect("first project should be created");

    assert!(matches!(
        store.create_project(parent.path(), &name),
        Err(StoreError::AlreadyExists(_))
    ));
}

#[test]
fn reports_a_non_directory_as_such_rather_than_as_an_io_failure() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let file = directory.path().join("notes.md");
    fs::write(&file, "notes").expect("notes file");

    assert!(matches!(
        FsProjectStore.open_project(&file),
        Err(StoreError::NotDirectory(_))
    ));
}

#[test]
fn reports_binary_files_as_invalid_text_rather_than_as_io_failures() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::write(directory.path().join("cover.txt"), [0xffu8, 0xfe, 0x00]).expect("binary file");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    assert!(matches!(
        store.read_document(&project, Path::new("cover.txt")),
        Err(StoreError::NotUtf8(_))
    ));
}

#[test]
fn refuses_to_open_a_document_above_the_editing_limit() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("huge.txt");
    // Sized rather than written, so the test costs no disk space.
    fs::File::create(&path)
        .expect("large file")
        .set_len(lantern_store::MAX_DOCUMENT_BYTES + 1)
        .expect("resize large file");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    assert!(matches!(
        store.read_document(&project, Path::new("huge.txt")),
        Err(StoreError::DocumentTooLarge { .. })
    ));
}

#[test]
fn saves_over_an_existing_document() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::write(directory.path().join("chapter.md"), "first draft").expect("document file");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    store
        .save_document(&project, Path::new("chapter.md"), "second draft")
        .expect("document should save");

    assert_eq!(
        fs::read_to_string(directory.path().join("chapter.md")).expect("read back"),
        "second draft"
    );
}

#[test]
fn refuses_to_save_outside_the_project() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    assert!(matches!(
        store.save_document(&project, Path::new("../escaped.md"), "text"),
        Err(StoreError::UnsafeProjectPath(_))
    ));
    assert!(matches!(
        store.save_document(&project, Path::new(".lantern/state.txt"), "text"),
        Err(StoreError::UnsafeProjectPath(_))
    ));
}

#[cfg(unix)]
#[test]
fn treats_a_symlinked_directory_as_a_directory() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(directory.path().join("chapters")).expect("chapters directory");
    std::os::unix::fs::symlink(
        directory.path().join("chapters"),
        directory.path().join("current"),
    )
    .expect("symlink");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    let entries = store
        .list_directory(&project, Path::new(""))
        .expect("list project root");
    let linked = entries
        .iter()
        .find(|entry| entry.name() == "current")
        .expect("symlink should be listed");

    assert!(linked.is_directory());
    assert!(
        store
            .list_directory(&project, linked.relative_path())
            .is_ok()
    );
}

#[cfg(unix)]
#[test]
fn a_non_utf8_file_name_stays_openable() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let file_name = OsStr::from_bytes(b"caf\xe9.md");
    fs::write(directory.path().join(file_name), "Once upon a time").expect("document file");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    let entries = store
        .list_directory(&project, Path::new(""))
        .expect("list project root");

    assert_eq!(entries.len(), 1);
    let document = store
        .read_document(&project, entries[0].relative_path())
        .expect("a listed document must be readable");
    assert_eq!(document.content(), "Once upon a time");
}
