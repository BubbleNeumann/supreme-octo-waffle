use lantern_core::{Color, ThemeMode};
use lantern_store::{FsThemeStore, StoreError, ThemeStore};
use std::path::Path;

/// A theme file spelling every entry Lantern reads, in the shipped format.
const GRUVBUDDY: &str = r##"
[Main]
name = "Gruvbuddy"
mode = "dark"

[Base]
base = "#111111"
default = "#f2e5bc"
red = "#cc6666"
orange = "#de935f"
green = "#99cc99"
blue = "#81a2be"

[Palette]
text = "default"
highlight = "blue"

[GUI]
errorText = "red"

[Syntax]
background = "base"
"##;

fn write_theme(directory: &Path, file_name: &str, contents: &str) {
    std::fs::write(directory.join(file_name), contents).expect("theme file");
}

#[test]
fn reads_the_entries_that_feed_the_interface() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_theme(directory.path(), "gruvbuddy.toml", GRUVBUDDY);
    let store = FsThemeStore::new([directory.path().to_owned()]);

    let themes = store.list_themes().expect("theme listing");

    assert_eq!(themes.len(), 1);
    let theme = &themes[0];
    assert_eq!(theme.name(), "Gruvbuddy");
    assert_eq!(theme.mode(), ThemeMode::Dark);
    let palette = theme.palette();
    assert_eq!(palette.background, Color::from_rgb(0x11, 0x11, 0x11));
    assert_eq!(palette.text, Color::from_rgb(0xf2, 0xe5, 0xbc));
    assert_eq!(palette.primary, Color::from_rgb(0x81, 0xa2, 0xbe));
    assert_eq!(palette.success, Color::from_rgb(0x99, 0xcc, 0x99));
    assert_eq!(palette.warning, Color::from_rgb(0xde, 0x93, 0x5f));
    assert_eq!(palette.danger, Color::from_rgb(0xcc, 0x66, 0x66));
}

#[test]
fn ignores_files_that_are_not_themes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_theme(directory.path(), "gruvbuddy.toml", GRUVBUDDY);
    write_theme(directory.path(), "notes.md", "not a theme");
    let store = FsThemeStore::new([directory.path().to_owned()]);

    let themes = store.list_themes().expect("theme listing");

    assert_eq!(themes.len(), 1);
}

#[test]
fn orders_themes_by_name_regardless_of_file_name() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_theme(directory.path(), "zzz.toml", GRUVBUDDY);
    write_theme(
        directory.path(),
        "aaa.toml",
        &GRUVBUDDY.replace("Gruvbuddy", "Solarized"),
    );
    let store = FsThemeStore::new([directory.path().to_owned()]);

    let themes = store.list_themes().expect("theme listing");

    assert_eq!(
        themes.iter().map(|theme| theme.name()).collect::<Vec<_>>(),
        vec!["Gruvbuddy", "Solarized"]
    );
}

#[test]
fn searches_the_first_directory_that_exists() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_theme(directory.path(), "gruvbuddy.toml", GRUVBUDDY);
    let store = FsThemeStore::new([
        directory.path().join("missing"),
        directory.path().to_owned(),
    ]);

    let themes = store.list_themes().expect("theme listing");

    assert_eq!(themes.len(), 1);
}

#[test]
fn offers_no_themes_when_no_search_path_exists() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = FsThemeStore::new([directory.path().join("missing")]);

    let themes = store.list_themes().expect("theme listing");

    assert!(themes.is_empty());
}

#[test]
fn falls_back_to_another_spelling_of_an_entry() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_theme(
        directory.path(),
        "partial.toml",
        &GRUVBUDDY.replace("errorText = \"red\"", ""),
    );
    let store = FsThemeStore::new([directory.path().to_owned()]);

    let themes = store.list_themes().expect("theme listing");

    // No [GUI] errorText, so the danger role comes from the base red instead.
    assert_eq!(
        themes[0].palette().danger,
        Color::from_rgb(0xcc, 0x66, 0x66)
    );
}

#[test]
fn reports_a_theme_that_omits_an_entry_it_needs() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_theme(
        directory.path(),
        "partial.toml",
        &GRUVBUDDY.replace("green = \"#99cc99\"", ""),
    );
    let store = FsThemeStore::new([directory.path().to_owned()]);

    let error = store.list_themes().expect_err("incomplete theme");

    assert!(
        matches!(&error, StoreError::ThemeKeyMissing { key, .. } if key == "Base.green"),
        "unexpected error: {error}"
    );
}

#[test]
fn reports_a_theme_naming_a_colour_it_never_defined() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_theme(
        directory.path(),
        "undefined.toml",
        &GRUVBUDDY.replace("highlight = \"blue\"", "highlight = \"chartreuse\""),
    );
    let store = FsThemeStore::new([directory.path().to_owned()]);

    let error = store.list_themes().expect_err("undefined colour");

    assert!(
        matches!(&error, StoreError::ThemeColor { key, .. } if key == "Palette.highlight"),
        "unexpected error: {error}"
    );
}

#[test]
fn reports_a_theme_that_is_not_valid_toml() {
    let directory = tempfile::tempdir().expect("temporary directory");
    write_theme(directory.path(), "broken.toml", "[Main\nname =");
    let store = FsThemeStore::new([directory.path().to_owned()]);

    let error = store.list_themes().expect_err("malformed theme");

    assert!(
        matches!(error, StoreError::ThemeSyntax { .. }),
        "unexpected error: {error}"
    );
}
