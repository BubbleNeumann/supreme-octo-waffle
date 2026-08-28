use lantern_core::{ProjectName, ProjectNameError};

#[test]
fn accepts_a_portable_project_name() {
    let name = ProjectName::new("  My Novel  ").expect("name should be valid");

    assert_eq!(name.as_str(), "My Novel");
}

#[test]
fn rejects_paths_and_reserved_names() {
    assert_eq!(
        ProjectName::new("books/novel"),
        Err(ProjectNameError::NotSingleComponent)
    );
    assert_eq!(
        ProjectName::new("CON"),
        Err(ProjectNameError::Reserved("CON".to_owned()))
    );
}

#[test]
fn rejects_names_with_no_visible_characters() {
    assert_eq!(ProjectName::new(""), Err(ProjectNameError::Empty));
    assert_eq!(ProjectName::new("   "), Err(ProjectNameError::Empty));
}

#[test]
fn rejects_characters_that_are_not_portable() {
    for name in [
        "chapter?", "a<b", "a>b", "a:b", "a\"b", "a|b", "a*b", "a\tb",
    ] {
        assert!(
            matches!(
                ProjectName::new(name),
                Err(ProjectNameError::InvalidCharacter(_))
            ),
            "{name:?} should be rejected"
        );
    }
}

#[test]
fn rejects_a_trailing_period_that_windows_would_drop() {
    assert_eq!(
        ProjectName::new("My Novel."),
        Err(ProjectNameError::TrailingPeriod)
    );
}

#[test]
fn rejects_reserved_device_names_with_an_extension() {
    assert_eq!(
        ProjectName::new("com1.txt"),
        Err(ProjectNameError::Reserved("com1.txt".to_owned()))
    );
    assert_eq!(
        ProjectName::new(".."),
        Err(ProjectNameError::NotSingleComponent)
    );
}

#[test]
fn accepts_a_name_that_merely_starts_with_a_device_name() {
    assert!(ProjectName::new("Console").is_ok());
    assert!(ProjectName::new("COM10").is_ok());
}
