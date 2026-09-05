use lantern_service::{FsProjectService, Project, ProjectService};
use std::path::{Path, PathBuf};

/// Opens `directory` as a project with the documents named written into it.
fn project(directory: &Path, documents: &[(&str, &str)]) -> (FsProjectService, Project) {
    for (relative_path, text) in documents {
        let path = directory.join(relative_path);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("document directory");
        }

        std::fs::write(&path, text).expect("document file");
    }

    let service = ProjectService::filesystem();
    let project = service
        .open_project(directory)
        .expect("project should open");

    (service, project)
}

/// Returns the text of one document in a project.
fn text(project: &Project, relative_path: &str) -> String {
    std::fs::read_to_string(project.root().join(relative_path)).expect("document should be read")
}

/// Saves `content` over a document the way the editor does.
fn save(service: &FsProjectService, project: &Project, relative_path: &str, content: &str) {
    let mut document = service
        .open_document(project, Path::new(relative_path))
        .expect("document should open");

    service
        .save_document(project, &mut document, content)
        .expect("document should save");
}

#[test]
fn a_chapter_with_nothing_under_it_is_an_ordinary_document() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(directory.path(), &[("chapters/Arrival.md", "A beginning")]);

    save(&service, &project, "chapters/Arrival.md", "A middle");

    assert_eq!(text(&project, "chapters/Arrival.md"), "A middle");
    assert!(
        service
            .scenes(&project, Path::new("chapters/Arrival.md"))
            .is_empty()
    );
    assert!(!directory.path().join("chapters").join("Arrival").exists());
}

#[test]
fn a_document_moved_under_a_chapter_becomes_the_scene_it_is_written_in() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", ""),
            ("drawer/The station.md", "Rain."),
        ],
    );

    let moved = service
        .move_document(
            &project,
            Path::new("drawer/The station.md"),
            Path::new("chapters/Arrival"),
        )
        .expect("scene should move under the chapter");

    assert_eq!(moved, PathBuf::from("chapters/Arrival/The station.md"));
    assert_eq!(text(&project, "chapters/Arrival/The station.md"), "Rain.");
    assert_eq!(text(&project, "chapters/Arrival.md"), "Rain.");
    assert_eq!(
        service.scenes(&project, Path::new("chapters/Arrival.md")),
        vec![PathBuf::from("chapters/Arrival/The station.md")]
    );
}

#[test]
fn a_chapter_that_was_already_written_keeps_its_text_as_the_first_scene() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "The train was late."),
            ("drawer/The house.md", "The gate stood open."),
        ],
    );

    service
        .move_document(
            &project,
            Path::new("drawer/The house.md"),
            Path::new("chapters/Arrival"),
        )
        .expect("scene should move under the chapter");

    assert_eq!(
        text(&project, "chapters/Arrival/Arrival.md"),
        "The train was late."
    );
    assert_eq!(
        text(&project, "chapters/Arrival.md"),
        "The train was late.\n--\nThe gate stood open."
    );
}

#[test]
fn saving_a_scene_writes_the_chapter_above_it_again() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One.\n--\nTwo."),
            ("chapters/Arrival/first.md", "One."),
            ("chapters/Arrival/second.md", "Two."),
        ],
    );

    save(
        &service,
        &project,
        "chapters/Arrival/second.md",
        "Two, rewritten.",
    );

    assert_eq!(
        text(&project, "chapters/Arrival.md"),
        "One.\n--\nTwo, rewritten."
    );
}

#[test]
fn saving_a_chapter_writes_its_text_back_over_its_scenes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One.\n--\nTwo."),
            ("chapters/Arrival/first.md", "One."),
            ("chapters/Arrival/second.md", "Two."),
        ],
    );

    save(
        &service,
        &project,
        "chapters/Arrival.md",
        "One, rewritten.\n--\nTwo.",
    );

    assert_eq!(
        text(&project, "chapters/Arrival/first.md"),
        "One, rewritten."
    );
    assert_eq!(text(&project, "chapters/Arrival/second.md"), "Two.");
}

#[test]
fn a_separator_written_into_a_chapter_becomes_another_scene() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One."),
            ("chapters/Arrival/first.md", "One."),
        ],
    );

    save(&service, &project, "chapters/Arrival.md", "One.\n--\nTwo.");

    assert_eq!(text(&project, "chapters/Arrival/first.md"), "One.");
    assert_eq!(text(&project, "chapters/Arrival/Scene 1.md"), "Two.");
    assert_eq!(
        service.scenes(&project, Path::new("chapters/Arrival.md")),
        vec![
            PathBuf::from("chapters/Arrival/first.md"),
            PathBuf::from("chapters/Arrival/Scene 1.md"),
        ]
    );
}

#[test]
fn a_separator_taken_out_of_a_chapter_joins_the_two_scenes_it_divided() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One.\n--\nTwo."),
            ("chapters/Arrival/first.md", "One."),
            ("chapters/Arrival/second.md", "Two."),
        ],
    );

    save(&service, &project, "chapters/Arrival.md", "One. Two.");

    assert_eq!(text(&project, "chapters/Arrival/first.md"), "One. Two.");
    assert!(
        !directory
            .path()
            .join("chapters")
            .join("Arrival")
            .join("second.md")
            .exists()
    );
    assert_eq!(text(&project, "chapters/Arrival.md"), "One. Two.");
}

#[test]
fn the_order_the_scenes_are_dragged_into_is_the_order_the_chapter_reads_in() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One.\n--\nTwo."),
            ("chapters/Arrival/first.md", "One."),
            ("chapters/Arrival/second.md", "Two."),
        ],
    );

    service
        .place_document(
            &project,
            Path::new("chapters/Arrival/second.md"),
            Path::new("chapters/Arrival"),
            Some("first.md"),
        )
        .expect("scene should be placed");

    assert_eq!(text(&project, "chapters/Arrival.md"), "Two.\n--\nOne.");
}

#[test]
fn a_scene_dragged_out_from_under_a_chapter_leaves_the_chapter_reading_on() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One.\n--\nTwo."),
            ("chapters/Arrival/first.md", "One."),
            ("chapters/Arrival/second.md", "Two."),
        ],
    );

    service
        .move_document(
            &project,
            Path::new("chapters/Arrival/second.md"),
            Path::new("drawer"),
        )
        .expect("scene should move out");

    assert_eq!(text(&project, "chapters/Arrival.md"), "One.");
    assert_eq!(text(&project, "drawer/second.md"), "Two.");
}

#[test]
fn the_last_scene_taken_out_leaves_the_chapter_as_it_stood() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One."),
            ("chapters/Arrival/first.md", "One."),
        ],
    );

    service
        .move_document(
            &project,
            Path::new("chapters/Arrival/first.md"),
            Path::new("drawer"),
        )
        .expect("scene should move out");

    assert_eq!(text(&project, "chapters/Arrival.md"), "One.");
    assert!(
        service
            .scenes(&project, Path::new("chapters/Arrival.md"))
            .is_empty()
    );
}

#[test]
fn a_scene_created_under_a_chapter_is_written_into_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One."),
            ("chapters/Arrival/first.md", "One."),
        ],
    );

    service
        .create_document(&project, Path::new("chapters/Arrival"), "second")
        .expect("scene should be created");

    assert_eq!(text(&project, "chapters/Arrival.md"), "One.\n--\n");
}

#[test]
fn a_chapters_own_line_endings_survive_being_written_from_its_scenes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/Arrival.md", "One.\r\n"),
            ("chapters/Arrival/first.md", "One.\n"),
        ],
    );

    save(
        &service,
        &project,
        "chapters/Arrival/first.md",
        "One.\nMore.\n",
    );

    assert_eq!(text(&project, "chapters/Arrival.md"), "One.\r\nMore.\r\n");
}

#[test]
fn a_document_under_something_that_is_not_a_chapter_is_an_ordinary_document() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (service, project) = project(
        directory.path(),
        &[
            ("chapters/act one/Arrival.md", "One."),
            ("references/Places.md", "Here."),
        ],
    );

    save(&service, &project, "chapters/act one/Arrival.md", "Two.");
    save(&service, &project, "references/Places.md", "There.");

    assert_eq!(text(&project, "chapters/act one/Arrival.md"), "Two.");
    assert_eq!(text(&project, "references/Places.md"), "There.");
    assert!(!directory.path().join("chapters").join("act.md").exists());
}
