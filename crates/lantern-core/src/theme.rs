//! Colour vocabulary for Lantern's interface themes.
//!
//! Themes are authored as files, but nothing here reads one. This module owns
//! the colour values and the small grammar their entries are written in, so
//! that storage only has to decide which file entry feeds which role.

use std::collections::HashMap;
use thiserror::Error;

/// A colour with eight bits per channel and straight alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    /// Creates an opaque colour from its red, green and blue channels.
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::from_rgba(red, green, blue, u8::MAX)
    }

    /// Creates a colour from its red, green, blue and alpha channels.
    pub const fn from_rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Parses an opaque `#rrggbb` or a translucent `#rrggbbaa` colour literal.
    pub fn from_hex(literal: &str) -> Result<Self, ThemeError> {
        let digits = literal
            .strip_prefix('#')
            .ok_or_else(|| ThemeError::MalformedColor(literal.to_owned()))?;

        if digits.len() != 6 && digits.len() != 8 {
            return Err(ThemeError::MalformedColor(literal.to_owned()));
        }

        let mut channels = [u8::MAX; 4];

        for (channel, digits) in channels.iter_mut().zip(digits.as_bytes().chunks(2)) {
            let pair = std::str::from_utf8(digits)
                .map_err(|_| ThemeError::MalformedColor(literal.to_owned()))?;

            *channel = u8::from_str_radix(pair, 16)
                .map_err(|_| ThemeError::MalformedColor(literal.to_owned()))?;
        }

        Ok(Self::from_rgba(
            channels[0],
            channels[1],
            channels[2],
            channels[3],
        ))
    }

    /// Returns the red channel.
    pub const fn red(self) -> u8 {
        self.red
    }

    /// Returns the green channel.
    pub const fn green(self) -> u8 {
        self.green
    }

    /// Returns the blue channel.
    pub const fn blue(self) -> u8 {
        self.blue
    }

    /// Returns the alpha channel, where `255` is fully opaque.
    pub const fn alpha(self) -> u8 {
        self.alpha
    }

    /// Returns the same colour at a different opacity.
    pub const fn with_alpha(self, alpha: u8) -> Self {
        Self { alpha, ..self }
    }
}

/// The named colours that a theme's entries may refer to by name.
///
/// A theme file defines these once and then spells the rest of its entries as
/// references to them, so a palette can be retuned in one place.
#[derive(Debug, Default, Clone)]
pub struct BaseColors(HashMap<String, Color>);

impl BaseColors {
    /// Creates an empty set of named colours.
    pub fn new() -> Self {
        Self::default()
    }

    /// Defines one named colour, replacing any previous definition.
    pub fn define(&mut self, name: impl Into<String>, color: Color) {
        self.0.insert(name.into(), color);
    }

    /// Returns a colour defined under `name`.
    pub fn get(&self, name: &str) -> Option<Color> {
        self.0.get(name).copied()
    }

    /// Resolves one theme entry into a colour.
    ///
    /// An entry is either a `#rrggbb` / `#rrggbbaa` literal or the name of a
    /// base colour. Either form may carry a `:alpha` suffix holding a `0`-`255`
    /// opacity, so that `blue:128` is the base blue at half opacity.
    pub fn resolve(&self, entry: &str) -> Result<Color, ThemeError> {
        let (color, alpha) = match entry.split_once(':') {
            Some((color, alpha)) => {
                let alpha = alpha
                    .parse()
                    .map_err(|_| ThemeError::MalformedOpacity(entry.to_owned()))?;

                (color, Some(alpha))
            }
            None => (entry, None),
        };

        let color = if color.starts_with('#') {
            Color::from_hex(color)?
        } else {
            self.get(color)
                .ok_or_else(|| ThemeError::UndefinedColor(color.to_owned()))?
        };

        Ok(match alpha {
            Some(alpha) => color.with_alpha(alpha),
            None => color,
        })
    }
}

/// Whether a theme is drawn for light or dark surroundings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    /// Dark text on a light background.
    Light,
    /// Light text on a dark background.
    Dark,
}

impl ThemeMode {
    /// Reads the `light` or `dark` spelling used in theme files.
    pub fn parse(value: &str) -> Result<Self, ThemeError> {
        match value {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(ThemeError::UnknownMode(value.to_owned())),
        }
    }
}

/// The colour roles every Lantern widget is drawn from.
///
/// These are deliberately few. The interface derives its surfaces, hovers and
/// borders from them, so a theme file's remaining entries have nowhere to go
/// until the widgets that would use them exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    /// The colour behind the editor and the explorer.
    pub background: Color,
    /// The colour of ordinary text on `background`.
    pub text: Color,
    /// The accent behind selections and the open document.
    pub primary: Color,
    /// The colour reporting that something succeeded.
    pub success: Color,
    /// The colour reporting that something needs attention.
    pub warning: Color,
    /// The colour reporting that something failed.
    pub danger: Color,
}

/// A named set of interface colours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    name: String,
    mode: ThemeMode,
    palette: ThemePalette,
}

impl Theme {
    /// Creates a theme from a display name and its resolved colours.
    pub fn new(name: impl Into<String>, mode: ThemeMode, palette: ThemePalette) -> Self {
        Self {
            name: name.into(),
            mode,
            palette,
        }
    }

    /// Returns the theme's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether the theme is light or dark.
    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// Returns the colours the interface draws from.
    pub fn palette(&self) -> ThemePalette {
        self.palette
    }
}

/// A failure while interpreting a theme's colours.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThemeError {
    /// A colour entry was neither a known name nor a `#rrggbb` literal.
    #[error("'{0}' is not a #rrggbb or #rrggbbaa colour")]
    MalformedColor(String),
    /// A colour entry carried a `:alpha` suffix outside `0`-`255`.
    #[error("'{0}' does not end in an opacity between 0 and 255")]
    MalformedOpacity(String),
    /// A colour entry named a colour the theme never defined.
    #[error("no base colour named '{0}' is defined")]
    UndefinedColor(String),
    /// A theme declared a mode other than `light` or `dark`.
    #[error("'{0}' is not a light or dark theme mode")]
    UnknownMode(String),
}
