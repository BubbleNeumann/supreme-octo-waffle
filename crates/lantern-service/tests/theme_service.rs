use lantern_service::{ThemeMode, ThemeService, ThemeServiceError};

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

#[test]
fn loads_an_installed_theme_by_name() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("gruvbuddy.toml"), GRUVBUDDY).expect("theme file");
    let service = ThemeService::filesystem([directory.path().to_owned()]);

    let theme = service.theme("Gruvbuddy").expect("installed theme");

    assert_eq!(theme.name(), "Gruvbuddy");
    assert_eq!(theme.mode(), ThemeMode::Dark);
}

#[test]
fn reports_a_theme_that_is_not_installed() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("gruvbuddy.toml"), GRUVBUDDY).expect("theme file");
    let service = ThemeService::filesystem([directory.path().to_owned()]);

    let error = service.theme("Solarized").expect_err("missing theme");

    assert!(
        matches!(&error, ThemeServiceError::UnknownTheme(name) if name == "Solarized"),
        "unexpected error: {error}"
    );
}

#[test]
fn offers_the_installed_themes_for_choosing() {
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("gruvbuddy.toml"), GRUVBUDDY).expect("theme file");
    std::fs::write(
        directory.path().join("solarized.toml"),
        GRUVBUDDY.replace("Gruvbuddy", "Solarized"),
    )
    .expect("theme file");
    let service = ThemeService::filesystem([directory.path().to_owned()]);

    let themes = service.available_themes().expect("theme listing");

    assert_eq!(
        themes.iter().map(|theme| theme.name()).collect::<Vec<_>>(),
        vec!["Gruvbuddy", "Solarized"]
    );
}
