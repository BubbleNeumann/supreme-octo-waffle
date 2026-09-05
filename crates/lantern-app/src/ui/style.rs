use super::text_editor;
use iced::widget::{button, container, text_input};
use iced::{Background, Border, Theme};

pub(super) fn sidebar_background(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        ..container::Style::default()
    }
}

pub(super) fn square_button(theme: &Theme, status: button::Status) -> button::Style {
    let style = button::subtle(theme, status);

    button::Style {
        border: Border {
            radius: 0.0.into(),
            ..style.border
        },
        ..style
    }
}

pub(super) fn tree_button(
    theme: &Theme,
    status: button::Status,
    drop_target: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    let mut style = button::subtle(theme, status);
    style.border = Border::default();

    // The directory a dragged document would land in, drawn the way the open
    // document is drawn: the one row in the tree currently being acted on.
    if drop_target {
        style.background = Some(palette.primary.weak.color.into());
        style.text_color = palette.primary.weak.text;
    }

    style
}

pub(super) fn file_button(
    theme: &Theme,
    status: button::Status,
    selected: bool,
    hovered: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    let mut style = button::subtle(theme, status);
    style.border = Border::default();

    // `hovered` is the pointer being anywhere in the row rather than over this
    // control: a chapter's row is drawn as two buttons against each other, and
    // half a row lighting up would not read as one row.
    if selected {
        style.background = Some(palette.primary.weak.color.into());
        style.text_color = palette.primary.weak.text;
    } else if hovered || matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = Some(palette.background.weak.color.into());
        style.text_color = palette.background.weak.text;
    }

    style
}

/// Draws the line marking where a dragged document would land.
pub(super) fn insertion_line(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.primary.base.color.into()),
        ..container::Style::default()
    }
}

pub(super) fn borderless_text_input(
    theme: &Theme,
    _status: text_input::Status,
) -> text_input::Style {
    let palette = theme.extended_palette();

    text_input::Style {
        background: Background::Color(palette.background.weak.color),
        border: Border::default(),
        icon: palette.background.weak.text,
        placeholder: palette.secondary.base.color,
        value: palette.background.weak.text,
        selection: palette.primary.weak.color,
    }
}

pub(super) fn borderless_editor(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let palette = theme.extended_palette();

    text_editor::Style {
        background: Background::Color(palette.background.base.color),
        border: Border::default(),
        placeholder: palette.secondary.base.color,
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
        // The theme's primary colour, which a Lantern theme file names as its
        // accent. The character the caret covers is drawn in the page's own
        // colour, so that the caret reads as the page and the text trading
        // places for one character, and the character is as legible on the
        // caret as the page is under the text.
        caret: palette.primary.base.color,
        caret_text: palette.background.base.color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lowest contrast Lantern will draw the caret's character at.
    ///
    /// Body text is usually held to 4.5. The caret's character is one glyph the
    /// eye is looking for rather than a paragraph, and trading places with the
    /// page clears this comfortably, so there is no reason to settle for less.
    const REQUIRED_LEGIBILITY: f32 = 7.0;

    /// The lowest contrast at which the caret still reads against the page.
    const REQUIRED_VISIBILITY: f32 = 3.0;

    #[test]
    fn the_editor_caret_is_drawn_in_a_colour_the_theme_file_names() {
        let theme = shipped_theme();

        let style = borderless_editor(&theme, text_editor::Status::Active);

        // Gruvbuddy's accent, and not the colour the text is drawn in.
        assert_eq!(style.caret, theme.palette().primary);
        assert_ne!(style.caret, style.value);
    }

    #[test]
    fn the_character_under_the_caret_stays_legible_on_it() {
        let style = shipped_editor_style();

        // The character is read against the caret rather than against the
        // page. Leaving it in the page's text colour would put it near 2.
        let legibility = contrast(style.caret_text, style.caret);

        assert!(
            legibility >= REQUIRED_LEGIBILITY,
            "the caret's character measured {legibility:.2} against it"
        );
        assert!(
            legibility > contrast(style.value, style.caret),
            "recolouring the character did not make it more legible"
        );
    }

    #[test]
    fn the_editors_page_is_the_colour_the_window_is() {
        let theme = shipped_theme();

        // With no project open the pane holds no editor, so what shows through
        // is the window's own background rather than the page the editor
        // paints. They have to be the same colour, or opening a project would
        // change the shade of the pane.
        let window = iced::theme::Base::base(&theme).background_color;

        assert_eq!(page_color(&shipped_editor_style()), window);
    }

    #[test]
    fn the_caret_itself_stays_visible_against_the_page() {
        let style = shipped_editor_style();
        let visibility = contrast(style.caret, page_color(&style));

        assert!(
            visibility >= REQUIRED_VISIBILITY,
            "the caret measured {visibility:.2} against the page"
        );
    }

    /// The theme Lantern ships, as Iced draws it.
    fn shipped_theme() -> Theme {
        crate::theme::to_iced(
            &crate::theme::service()
                .theme(crate::theme::DEFAULT_THEME)
                .expect("default theme"),
        )
    }

    /// The editor's style under the theme Lantern ships.
    fn shipped_editor_style() -> text_editor::Style {
        borderless_editor(&shipped_theme(), text_editor::Status::Active)
    }

    /// The colour the editor draws its page in.
    fn page_color(style: &text_editor::Style) -> iced::Color {
        match style.background {
            Background::Color(color) => color,
            background => panic!("the editor's background is not a colour: {background:?}"),
        }
    }

    /// The relative luminance of a colour, per WCAG 2.
    fn luminance(color: iced::Color) -> f32 {
        fn channel(value: f32) -> f32 {
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }

    /// How legible one colour is against another, per WCAG 2.
    fn contrast(over: iced::Color, under: iced::Color) -> f32 {
        let (lighter, darker) = if luminance(over) > luminance(under) {
            (luminance(over), luminance(under))
        } else {
            (luminance(under), luminance(over))
        };

        (lighter + 0.05) / (darker + 0.05)
    }
}
