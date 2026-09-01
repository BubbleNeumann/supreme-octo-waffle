//! The block caret Lantern draws in place of Iced's one-pixel line.
//!
//! The widget that draws it is vendored in [`super::text_editor`]; this module
//! holds the part that is Lantern's own, so that it stays out of the vendored
//! file and can be tested on its own.

use iced::advanced::text::{self, LineHeight, Paragraph as _, Text, Wrapping};
use iced::{Pixels, Size, alignment};

/// The character the caret is measured against.
///
/// The editor is set in a monospaced face, so every character occupies the same
/// width and any one of them measures the caret. This one is arbitrary.
const REFERENCE_CHARACTER: char = 'M';

/// How wide one character is in the editor's font at `size`.
///
/// This is a character's advance, which in a monospaced face is every
/// character's advance, so the caret is one character wide wherever it sits and
/// whatever it is sitting on.
///
/// It is measured rather than assumed because the advance depends on the face
/// and on the size the editor is currently drawn at, both of which change: the
/// size on every zoom step.
pub(super) fn width<Renderer: text::Renderer>(
    font: Renderer::Font,
    size: Pixels,
    line_height: LineHeight,
) -> f32 {
    advance::<Renderer>(REFERENCE_CHARACTER, font, size, line_height)
}

/// How much room `character` takes when the editor lays it out.
fn advance<Renderer: text::Renderer>(
    character: char,
    font: Renderer::Font,
    size: Pixels,
    line_height: LineHeight,
) -> f32 {
    let mut encoded = [0; 4];

    Renderer::Paragraph::with_text(Text {
        content: &*character.encode_utf8(&mut encoded),
        bounds: Size::INFINITE,
        size,
        line_height,
        font,
        align_x: text::Alignment::Default,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: Wrapping::None,
    })
    .min_bounds()
    .width
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{EDITOR_FONT, EDITOR_FONT_BYTES};

    /// Registers the bundled face, as the application does before it opens.
    ///
    /// Nothing has loaded it in a test, so without this every measurement below
    /// would silently fall back to whatever face the system offers instead.
    fn load_editor_font() {
        iced::advanced::graphics::text::font_system()
            .write()
            .expect("the font system should be readable")
            .load_font(std::borrow::Cow::Borrowed(EDITOR_FONT_BYTES));
    }

    /// Measures a character the way the editor lays it out.
    fn measure(character: char, size: Pixels) -> f32 {
        load_editor_font();

        advance::<iced::Renderer>(character, EDITOR_FONT, size, LineHeight::default())
    }

    /// The caret width the editor would draw at a given size.
    fn caret(size: Pixels) -> f32 {
        load_editor_font();

        width::<iced::Renderer>(EDITOR_FONT, size, LineHeight::default())
    }

    #[test]
    fn the_editor_font_gives_every_character_the_same_width() {
        // The caret is one width for the whole document, which only holds
        // because the face the editor is set in is monospaced.
        let reference = measure('M', Pixels(16.0));

        for character in ['i', 'W', ' ', '0', '.', 'g'] {
            assert_eq!(
                measure(character, Pixels(16.0)),
                reference,
                "{character:?} is not the same width as 'M'"
            );
        }
    }

    #[test]
    fn the_caret_is_one_character_wide() {
        assert_eq!(caret(Pixels(16.0)), measure('a', Pixels(16.0)));
    }

    #[test]
    fn the_caret_has_a_width_to_draw() {
        let width = caret(Pixels(16.0));

        assert!(width > 1.0, "the caret measured {width}");
    }

    #[test]
    fn the_caret_grows_with_the_editor_font() {
        assert!(caret(Pixels(24.0)) > caret(Pixels(12.0)));
    }
}
