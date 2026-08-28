use lantern_core::{Document, DocumentEncoding, LineEnding};
use std::path::PathBuf;

fn open(raw_content: &str) -> Document {
    Document::from_verified_content(PathBuf::from("chapter.md"), raw_content.to_owned())
}

#[test]
fn normalizes_windows_line_endings_for_editing() {
    let document = open("one\r\ntwo\r\n");

    assert_eq!(document.content(), "one\ntwo\n");
    assert_eq!(document.encoding().line_ending(), LineEnding::Crlf);
}

#[test]
fn restores_windows_line_endings_when_saving() {
    let document = open("one\r\ntwo\r\n");

    assert_eq!(
        document.encoding().apply(document.content()),
        "one\r\ntwo\r\n"
    );
}

#[test]
fn leaves_unix_line_endings_alone() {
    let document = open("one\ntwo\n");

    assert_eq!(document.encoding().line_ending(), LineEnding::Lf);
    assert_eq!(document.encoding().apply(document.content()), "one\ntwo\n");
}

#[test]
fn strips_and_restores_a_byte_order_mark() {
    let document = open("\u{feff}Once upon a time");

    assert_eq!(document.content(), "Once upon a time");
    assert!(document.encoding().has_byte_order_mark());
    assert_eq!(
        document.encoding().apply(document.content()),
        "\u{feff}Once upon a time"
    );
}

#[test]
fn an_unedited_document_round_trips_byte_for_byte() {
    for raw_content in [
        "",
        "no trailing newline",
        "one\ntwo\n",
        "one\r\ntwo\r\n",
        "\u{feff}one\r\ntwo\r\n",
        "\u{feff}one\ntwo",
    ] {
        let document = open(raw_content);

        assert_eq!(
            document.encoding().apply(document.content()),
            raw_content,
            "round trip changed {raw_content:?}"
        );
    }
}

#[test]
fn takes_the_convention_of_the_first_line_break() {
    let (encoding, content) = DocumentEncoding::detect("one\r\ntwo\nthree");

    assert_eq!(encoding.line_ending(), LineEnding::Crlf);
    assert_eq!(content, "one\ntwo\nthree");
}

#[test]
fn an_empty_document_saves_as_nothing() {
    let document = open("");

    assert_eq!(document.content(), "");
    assert_eq!(document.encoding().apply(""), "");
}
