use lantern_core::{DocumentName, DocumentNameError};

#[test]
fn gives_a_plain_title_the_markdown_extension() {
    let name = DocumentName::new("  Chapter One  ").expect("name should be valid");

    assert_eq!(name.as_str(), "Chapter One.md");
}

#[test]
fn keeps_an_extension_lantern_can_open() {
    for typed in ["one.md", "one.markdown", "one.txt", "one.MD"] {
        let name = DocumentName::new(typed).expect("name should be valid");

        assert_eq!(name.as_str(), typed);
    }
}

#[test]
fn treats_a_title_holding_a_period_as_a_title() {
    let name = DocumentName::new("Mrs. Dalloway").expect("name should be valid");

    assert_eq!(name.as_str(), "Mrs. Dalloway.md");
}

#[test]
fn rejects_paths_and_reserved_names() {
    assert_eq!(
        DocumentName::new("chapters/one.md"),
        Err(DocumentNameError::NotSingleComponent)
    );
    assert_eq!(
        DocumentName::new("NUL.md"),
        Err(DocumentNameError::Reserved("NUL.md".to_owned()))
    );
}

#[test]
fn rejects_names_with_no_visible_characters() {
    assert_eq!(DocumentName::new(""), Err(DocumentNameError::Empty));
    assert_eq!(DocumentName::new("   "), Err(DocumentNameError::Empty));
}

#[test]
fn rejects_characters_that_are_not_portable() {
    for typed in ["one?.md", "a<b", "a|b.txt"] {
        assert!(
            matches!(
                DocumentName::new(typed),
                Err(DocumentNameError::InvalidCharacter(_))
            ),
            "{typed:?} should be rejected"
        );
    }
}

#[test]
fn rejects_a_trailing_period_that_windows_would_drop() {
    assert_eq!(
        DocumentName::new("Chapter One."),
        Err(DocumentNameError::TrailingPeriod)
    );
}
