use lantern_core::{ProjectEntry, ProjectEntryKind, WORKSPACE_DIRECTORIES};
use std::path::PathBuf;

fn entry(name: &str, kind: ProjectEntryKind) -> ProjectEntry {
    ProjectEntry::from_verified_path(PathBuf::from(name), name.to_owned(), kind)
}

#[test]
fn matches_a_directory_by_name_whatever_case_it_was_created_in() {
    let directory = entry("Chapters", ProjectEntryKind::Directory);

    assert!(directory.is_directory_named("chapters"));
    assert!(directory.is_directory_named("CHAPTERS"));
    assert!(!directory.is_directory_named("drawer"));
}

#[test]
fn a_file_is_never_a_directory_however_it_is_named() {
    let file = entry("chapters", ProjectEntryKind::File);

    assert!(!file.is_directory_named("chapters"));
}

#[test]
fn the_workspace_directories_are_the_ones_a_project_opens_onto() {
    assert_eq!(WORKSPACE_DIRECTORIES, ["chapters", "references", "drawer"]);
}
