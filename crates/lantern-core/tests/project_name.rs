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
