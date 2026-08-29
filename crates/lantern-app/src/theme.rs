//! Turning Lantern's interface themes into the palette Iced draws from.
//!
//! Every widget style in `ui::style` reads Iced's extended palette, which Iced
//! derives from six roles. Mapping a theme onto those roles is therefore the
//! whole of applying it — the styles themselves need no theme knowledge.

use iced::theme::Palette;
use lantern_service::{Color, FsThemeService, Theme, ThemeService};
use std::path::PathBuf;

/// The theme Lantern starts in when it is installed alongside one.
pub(crate) const DEFAULT_THEME: &str = "Gruvbuddy";

/// The directory name holding Lantern's theme files.
const THEMES_DIRECTORY: &str = "themes";

/// Creates the theme service, searching where themes are kept.
///
/// An installed Lantern keeps them beside the executable; a checkout keeps them
/// in the workspace, which is where a `cargo run` build finds them.
pub(crate) fn service() -> FsThemeService {
    let mut search_paths = Vec::new();

    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        search_paths.push(directory.join(THEMES_DIRECTORY));
    }

    search_paths.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(THEMES_DIRECTORY),
    );

    ThemeService::filesystem(search_paths)
}

/// Converts a Lantern theme into the Iced theme the widgets are drawn with.
pub(crate) fn to_iced(theme: &Theme) -> iced::Theme {
    let palette = theme.palette();

    iced::Theme::custom(
        theme.name().to_owned(),
        Palette {
            background: to_color(palette.background),
            text: to_color(palette.text),
            primary: to_color(palette.primary),
            success: to_color(palette.success),
            warning: to_color(palette.warning),
            danger: to_color(palette.danger),
        },
    )
}

/// Converts one Lantern colour into an Iced colour.
fn to_color(color: Color) -> iced::Color {
    iced::Color::from_rgba8(
        color.red(),
        color.green(),
        color.blue(),
        f32::from(color.alpha()) / f32::from(u8::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_themes_directory_provides_the_default_theme() {
        let theme = service()
            .theme(DEFAULT_THEME)
            .expect("the workspace ships the default theme");

        assert_eq!(theme.name(), DEFAULT_THEME);
    }

    #[test]
    fn a_theme_becomes_the_palette_iced_draws_from() {
        let theme = service().theme(DEFAULT_THEME).expect("default theme");

        let palette = to_iced(&theme).palette();

        assert_eq!(palette.background, iced::Color::from_rgb8(0x11, 0x11, 0x11));
        assert_eq!(palette.text, iced::Color::from_rgb8(0xf2, 0xe5, 0xbc));
        assert_eq!(palette.primary, iced::Color::from_rgb8(0x81, 0xa2, 0xbe));
        assert_eq!(palette.danger, iced::Color::from_rgb8(0xcc, 0x66, 0x66));
    }

    #[test]
    fn a_translucent_entry_keeps_its_opacity() {
        let color = to_color(lantern_service::Color::from_rgba(0x81, 0xa2, 0xbe, 128));

        assert!((color.a - 128.0 / 255.0).abs() < f32::EPSILON);
    }
}
