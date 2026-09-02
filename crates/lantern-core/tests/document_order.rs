use lantern_core::{ProjectEntry, ProjectEntryKind, order_documents};

/// Builds a listing in the sequence storage would have reported it.
fn listing(names: &[(&str, ProjectEntryKind)]) -> Vec<ProjectEntry> {
    names
        .iter()
        .map(|(name, kind)| {
            ProjectEntry::from_verified_path(name.into(), (*name).to_owned(), *kind)
        })
        .collect()
}

/// Returns the names a listing draws, top to bottom.
fn names(entries: &[ProjectEntry]) -> Vec<&str> {
    entries.iter().map(ProjectEntry::name).collect()
}

#[test]
fn draws_documents_in_the_order_the_author_gave() {
    let entries = listing(&[
        ("one.md", ProjectEntryKind::File),
        ("three.md", ProjectEntryKind::File),
        ("two.md", ProjectEntryKind::File),
    ]);

    let ordered = order_documents(
        entries,
        &[
            "three.md".to_owned(),
            "one.md".to_owned(),
            "two.md".to_owned(),
        ],
    );

    assert_eq!(names(&ordered), vec!["three.md", "one.md", "two.md"]);
}

#[test]
fn keeps_directories_ahead_of_the_documents() {
    let entries = listing(&[
        ("act-one", ProjectEntryKind::Directory),
        ("act-two", ProjectEntryKind::Directory),
        ("one.md", ProjectEntryKind::File),
        ("two.md", ProjectEntryKind::File),
    ]);

    let ordered = order_documents(entries, &["two.md".to_owned(), "one.md".to_owned()]);

    assert_eq!(
        names(&ordered),
        vec!["act-one", "act-two", "two.md", "one.md"]
    );
}

#[test]
fn draws_a_document_the_order_never_named_after_the_ones_it_did() {
    let entries = listing(&[
        ("added.md", ProjectEntryKind::File),
        ("one.md", ProjectEntryKind::File),
        ("two.md", ProjectEntryKind::File),
    ]);

    let ordered = order_documents(entries, &["two.md".to_owned(), "one.md".to_owned()]);

    assert_eq!(names(&ordered), vec!["two.md", "one.md", "added.md"]);
}

#[test]
fn documents_no_order_names_keep_the_sequence_storage_gave_them() {
    let entries = listing(&[
        ("a.md", ProjectEntryKind::File),
        ("b.md", ProjectEntryKind::File),
        ("c.md", ProjectEntryKind::File),
    ]);

    let ordered = order_documents(entries, &[]);

    assert_eq!(names(&ordered), vec!["a.md", "b.md", "c.md"]);
}

#[test]
fn an_order_naming_documents_that_are_gone_places_the_ones_that_remain() {
    let entries = listing(&[
        ("one.md", ProjectEntryKind::File),
        ("two.md", ProjectEntryKind::File),
    ]);

    let ordered = order_documents(
        entries,
        &[
            "deleted.md".to_owned(),
            "two.md".to_owned(),
            "one.md".to_owned(),
        ],
    );

    assert_eq!(names(&ordered), vec!["two.md", "one.md"]);
}
