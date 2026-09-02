use lantern_store::{FsProjectStore, ProjectStore};
use std::fs;
use std::path::Path;

#[test]
fn records_an_order_and_reads_it_back() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(directory.path().join("chapters")).expect("chapters directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");
    let order = vec!["two.md".to_owned(), "one.md".to_owned()];

    store
        .set_document_order(&project, Path::new("chapters"), &order)
        .expect("order should be recorded");

    assert_eq!(store.document_order(&project, Path::new("chapters")), order);
}

#[test]
fn keeps_each_directory_apart_in_the_one_file() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    store
        .set_document_order(&project, Path::new("chapters"), &["one.md".to_owned()])
        .expect("chapters order");
    store
        .set_document_order(&project, Path::new("drawer"), &["cut.md".to_owned()])
        .expect("drawer order");

    assert_eq!(
        store.document_order(&project, Path::new("chapters")),
        vec!["one.md".to_owned()]
    );
    assert_eq!(
        store.document_order(&project, Path::new("drawer")),
        vec!["cut.md".to_owned()]
    );
}

#[test]
fn an_order_is_kept_where_lantern_keeps_its_own_state() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    store
        .set_document_order(&project, Path::new("chapters"), &["one.md".to_owned()])
        .expect("order should be recorded");

    let recorded =
        fs::read_to_string(directory.path().join(".lantern").join("order.toml")).expect("read");
    // Spelled with forward slashes, so a project carried between systems keeps
    // the order it was given.
    assert!(recorded.contains("chapters"), "{recorded}");
    assert!(recorded.contains("one.md"), "{recorded}");
}

#[test]
fn a_nested_directory_is_recorded_under_its_own_path() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");
    let nested = Path::new("chapters").join("act-one");

    store
        .set_document_order(&project, &nested, &["arrival.md".to_owned()])
        .expect("order should be recorded");

    assert_eq!(
        store.document_order(&project, &nested),
        vec!["arrival.md".to_owned()]
    );
    assert!(
        store
            .document_order(&project, Path::new("chapters"))
            .is_empty()
    );
    let recorded =
        fs::read_to_string(directory.path().join(".lantern").join("order.toml")).expect("read");
    assert!(recorded.contains("chapters/act-one"), "{recorded}");
}

#[test]
fn an_empty_order_leaves_nothing_behind() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");
    store
        .set_document_order(&project, Path::new("chapters"), &["one.md".to_owned()])
        .expect("order should be recorded");

    store
        .set_document_order(&project, Path::new("chapters"), &[])
        .expect("order should be cleared");

    assert!(
        store
            .document_order(&project, Path::new("chapters"))
            .is_empty()
    );
    let recorded =
        fs::read_to_string(directory.path().join(".lantern").join("order.toml")).expect("read");
    assert!(!recorded.contains("one.md"), "{recorded}");
}

#[test]
fn a_directory_that_was_never_ordered_has_no_order() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    assert!(
        store
            .document_order(&project, Path::new("chapters"))
            .is_empty()
    );
}

#[test]
fn a_damaged_order_file_costs_the_order_rather_than_the_project() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(directory.path().join(".lantern")).expect("state directory");
    fs::write(
        directory.path().join(".lantern").join("order.toml"),
        "this is not [ TOML",
    )
    .expect("damaged order file");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");

    assert!(
        store
            .document_order(&project, Path::new("chapters"))
            .is_empty()
    );
    // And the project still lists, which is the point of tolerating it.
    assert!(store.list_directory(&project, Path::new("")).is_ok());
}

#[test]
fn an_order_survives_being_written_over_by_another_directory() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsProjectStore;
    let project = store.open_project(directory.path()).expect("open project");
    store
        .set_document_order(&project, Path::new("chapters"), &["one.md".to_owned()])
        .expect("chapters order");

    store
        .set_document_order(&project, Path::new("chapters"), &["two.md".to_owned()])
        .expect("chapters order again");

    assert_eq!(
        store.document_order(&project, Path::new("chapters")),
        vec!["two.md".to_owned()]
    );
}
