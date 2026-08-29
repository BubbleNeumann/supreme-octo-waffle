use lantern_core::{BaseColors, Color, ThemeError, ThemeMode};

#[test]
fn reads_an_opaque_six_digit_colour() {
    let color = Color::from_hex("#81a2be").expect("colour literal");

    assert_eq!(color, Color::from_rgb(0x81, 0xa2, 0xbe));
    assert_eq!(color.alpha(), u8::MAX);
}

#[test]
fn reads_the_alpha_channel_of_an_eight_digit_colour() {
    let color = Color::from_hex("#81a2be40").expect("colour literal");

    assert_eq!(color, Color::from_rgba(0x81, 0xa2, 0xbe, 0x40));
}

#[test]
fn rejects_colours_that_are_not_hexadecimal_literals() {
    assert_eq!(
        Color::from_hex("81a2be"),
        Err(ThemeError::MalformedColor("81a2be".to_owned()))
    );
    assert_eq!(
        Color::from_hex("#81a2b"),
        Err(ThemeError::MalformedColor("#81a2b".to_owned()))
    );
    assert_eq!(
        Color::from_hex("#81a2bz"),
        Err(ThemeError::MalformedColor("#81a2bz".to_owned()))
    );
}

#[test]
fn resolves_an_entry_naming_a_base_colour() {
    let mut base = BaseColors::new();
    base.define("blue", Color::from_rgb(0x81, 0xa2, 0xbe));

    assert_eq!(
        base.resolve("blue").expect("named colour"),
        Color::from_rgb(0x81, 0xa2, 0xbe)
    );
}

#[test]
fn applies_the_opacity_suffix_of_an_entry() {
    let mut base = BaseColors::new();
    base.define("yellow", Color::from_rgb(0xf8, 0xfe, 0x7a));

    assert_eq!(
        base.resolve("yellow:96").expect("named colour"),
        Color::from_rgba(0xf8, 0xfe, 0x7a, 96)
    );
    assert_eq!(
        base.resolve("#f8fe7a:96").expect("colour literal"),
        Color::from_rgba(0xf8, 0xfe, 0x7a, 96)
    );
}

#[test]
fn rejects_an_opacity_outside_a_single_byte() {
    let mut base = BaseColors::new();
    base.define("blue", Color::from_rgb(0x81, 0xa2, 0xbe));

    assert_eq!(
        base.resolve("blue:256"),
        Err(ThemeError::MalformedOpacity("blue:256".to_owned()))
    );
}

#[test]
fn rejects_an_entry_naming_a_colour_the_theme_never_defined() {
    let base = BaseColors::new();

    assert_eq!(
        base.resolve("chartreuse"),
        Err(ThemeError::UndefinedColor("chartreuse".to_owned()))
    );
}

#[test]
fn reads_the_light_and_dark_theme_modes() {
    assert_eq!(ThemeMode::parse("light"), Ok(ThemeMode::Light));
    assert_eq!(ThemeMode::parse("dark"), Ok(ThemeMode::Dark));
    assert_eq!(
        ThemeMode::parse("sepia"),
        Err(ThemeError::UnknownMode("sepia".to_owned()))
    );
}
