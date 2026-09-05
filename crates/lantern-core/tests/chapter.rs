use lantern_core::{
    SCENE_SEPARATOR, is_chapter, is_scene_directory_of, join_scenes, scene_directory, split_scenes,
    unused_scene_name,
};
use std::path::{Path, PathBuf};

#[test]
fn a_document_directly_in_the_chapters_directory_is_a_chapter() {
    assert!(is_chapter(Path::new("chapters/Arrival.md")));
    assert!(is_chapter(Path::new("chapters/Arrival.txt")));
}

#[test]
fn the_chapters_directory_is_recognised_however_it_is_spelled() {
    assert!(is_chapter(Path::new("Chapters/Arrival.md")));
    assert!(is_chapter(Path::new("CHAPTERS/Arrival.md")));
}

#[test]
fn a_document_under_a_chapter_is_not_itself_a_chapter() {
    assert!(!is_chapter(Path::new("chapters/Arrival/The station.md")));
}

#[test]
fn a_document_kept_anywhere_else_is_not_a_chapter() {
    assert!(!is_chapter(Path::new("references/Places.md")));
    assert!(!is_chapter(Path::new("drawer/Cut.md")));
    assert!(!is_chapter(Path::new("Arrival.md")));
}

#[test]
fn a_file_that_cannot_be_edited_is_not_a_chapter() {
    assert!(!is_chapter(Path::new("chapters/cover.png")));
}

#[test]
fn a_chapters_scenes_are_kept_in_a_directory_carrying_its_name() {
    assert_eq!(
        scene_directory(Path::new("chapters/Arrival.md")),
        Some(PathBuf::from("chapters/Arrival"))
    );

    assert!(is_scene_directory_of(
        Path::new("chapters/Arrival"),
        Path::new("chapters/Arrival.md")
    ));
}

#[test]
fn a_directory_beside_a_chapter_is_not_that_chapters_scenes() {
    assert!(!is_scene_directory_of(
        Path::new("chapters/act one"),
        Path::new("chapters/Arrival.md")
    ));
}

#[test]
fn a_chapter_is_its_scenes_joined_by_a_line_of_two_minus_signs() {
    assert_eq!(
        join_scenes(["The station.", "The house."]),
        "The station.\n--\nThe house."
    );
    assert_eq!(join_scenes(["Alone."]), "Alone.");
}

#[test]
fn splitting_a_chapter_returns_the_scenes_it_was_joined_from() {
    let scenes = ["The station.\n", "\nThe house.", ""];
    let chapter = join_scenes(scenes);

    assert_eq!(split_scenes(&chapter), scenes);
}

#[test]
fn a_chapter_with_no_separator_holds_one_scene() {
    assert_eq!(split_scenes("The station."), vec!["The station."]);
    assert_eq!(split_scenes(""), vec![""]);
}

#[test]
fn two_minus_signs_inside_a_line_do_not_divide_a_chapter() {
    let content = "She waited -- and waited.";

    assert_eq!(split_scenes(content), vec![content]);
    assert!(!content.contains(SCENE_SEPARATOR));
}

#[test]
fn a_created_scene_is_named_around_the_names_already_taken() {
    assert_eq!(unused_scene_name(&[]), "Scene 1.md");
    assert_eq!(
        unused_scene_name(&["Scene 1.md".to_owned(), "scene 2.MD".to_owned()]),
        "Scene 3.md"
    );
}
