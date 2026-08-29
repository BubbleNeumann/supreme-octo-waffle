//! Reading interface themes from TOML files.
//!
//! A theme file defines its named colours under `[Base]` and then spells the
//! rest of its entries as references to them. Lantern reads only the entries
//! that feed interface elements it actually draws; the rest of a file is
//! carried by themes written for other applications and is ignored until the
//! matching widgets exist.

use crate::StoreError;
use lantern_core::{BaseColors, Theme, ThemeMode, ThemePalette};
use std::fs;
use std::path::{Path, PathBuf};

/// The file extension Lantern recognises as a theme.
const THEME_EXTENSION: &str = "toml";

/// The table and key a theme's display name is read from.
const NAME_ENTRY: (&str, &str) = ("Main", "name");

/// The table and key a theme's light or dark mode is read from.
const MODE_ENTRY: (&str, &str) = ("Main", "mode");

/// The table holding a theme's named colours.
const BASE_TABLE: &str = "Base";

/// Where each palette role is read from, in order of preference.
///
/// Later candidates let a theme written against a slightly different vocabulary
/// still resolve, rather than failing over a key it spells another way.
const BACKGROUND_ENTRIES: &[(&str, &str)] = &[("Syntax", "background"), ("Palette", "base")];
const TEXT_ENTRIES: &[(&str, &str)] = &[("Palette", "text"), ("Syntax", "text")];
const PRIMARY_ENTRIES: &[(&str, &str)] = &[("Palette", "highlight"), ("Palette", "accent")];
const SUCCESS_ENTRIES: &[(&str, &str)] = &[("Base", "green")];
const WARNING_ENTRIES: &[(&str, &str)] = &[("Base", "orange"), ("Icon", "warning")];
const DANGER_ENTRIES: &[(&str, &str)] = &[("GUI", "errorText"), ("Base", "red")];

/// Reading the interface themes available to Lantern.
pub trait ThemeStore {
    /// Lists every readable theme, ordered by name.
    fn list_themes(&self) -> Result<Vec<Theme>, StoreError>;
}

/// Reads themes from the first of several directories that exists.
///
/// An installed Lantern keeps its themes beside the executable while a checkout
/// keeps them in the workspace, so the caller supplies both and the store uses
/// whichever is present.
#[derive(Debug, Clone)]
pub struct FsThemeStore {
    search_paths: Vec<PathBuf>,
}

impl FsThemeStore {
    /// Creates a store that searches the given directories in order.
    pub fn new(search_paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            search_paths: search_paths.into_iter().collect(),
        }
    }

    /// Returns the first search path that is a directory.
    fn theme_directory(&self) -> Option<&Path> {
        self.search_paths
            .iter()
            .map(PathBuf::as_path)
            .find(|path| path.is_dir())
    }
}

impl ThemeStore for FsThemeStore {
    fn list_themes(&self) -> Result<Vec<Theme>, StoreError> {
        let Some(directory) = self.theme_directory() else {
            return Ok(Vec::new());
        };

        let listing = fs::read_dir(directory).map_err(|source| StoreError::Io {
            path: directory.to_owned(),
            source,
        })?;
        let mut themes = Vec::new();

        for entry in listing {
            let entry = entry.map_err(|source| StoreError::Io {
                path: directory.to_owned(),
                source,
            })?;
            let path = entry.path();

            if path.extension().and_then(|extension| extension.to_str()) != Some(THEME_EXTENSION) {
                continue;
            }

            themes.push(read_theme(&path)?);
        }

        themes.sort_by_cached_key(|theme| theme.name().to_lowercase());

        Ok(themes)
    }
}

/// Reads and resolves one theme file.
fn read_theme(path: &Path) -> Result<Theme, StoreError> {
    let text = fs::read_to_string(path).map_err(|source| StoreError::Io {
        path: path.to_owned(),
        source,
    })?;
    let document: toml::Table = text.parse().map_err(|source| StoreError::ThemeSyntax {
        path: path.to_owned(),
        source,
    })?;

    let base = base_colors(&document, path)?;
    let name = entry(&document, path, NAME_ENTRY)?;
    let mode = ThemeMode::parse(&entry(&document, path, MODE_ENTRY)?).map_err(|source| {
        StoreError::ThemeColor {
            path: path.to_owned(),
            key: format!("{}.{}", MODE_ENTRY.0, MODE_ENTRY.1),
            source,
        }
    })?;

    let palette = ThemePalette {
        background: color(&document, &base, path, BACKGROUND_ENTRIES)?,
        text: color(&document, &base, path, TEXT_ENTRIES)?,
        primary: color(&document, &base, path, PRIMARY_ENTRIES)?,
        success: color(&document, &base, path, SUCCESS_ENTRIES)?,
        warning: color(&document, &base, path, WARNING_ENTRIES)?,
        danger: color(&document, &base, path, DANGER_ENTRIES)?,
    };

    Ok(Theme::new(name, mode, palette))
}

/// Collects the `[Base]` table's named colours.
fn base_colors(document: &toml::Table, path: &Path) -> Result<BaseColors, StoreError> {
    let mut base = BaseColors::new();

    let Some(table) = document.get(BASE_TABLE).and_then(toml::Value::as_table) else {
        return Ok(base);
    };

    for (name, value) in table {
        let Some(literal) = value.as_str() else {
            continue;
        };

        // Base entries are literals rather than references, so they resolve
        // against the colours defined before them.
        let color = base
            .resolve(literal)
            .map_err(|source| StoreError::ThemeColor {
                path: path.to_owned(),
                key: format!("{BASE_TABLE}.{name}"),
                source,
            })?;

        base.define(name.clone(), color);
    }

    Ok(base)
}

/// Reads one string entry, naming it in the error when it is absent.
fn entry(
    document: &toml::Table,
    path: &Path,
    (table, key): (&str, &str),
) -> Result<String, StoreError> {
    document
        .get(table)
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get(key))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| StoreError::ThemeKeyMissing {
            path: path.to_owned(),
            key: format!("{table}.{key}"),
        })
}

/// Resolves the first candidate entry a theme defines.
fn color(
    document: &toml::Table,
    base: &BaseColors,
    path: &Path,
    candidates: &[(&str, &str)],
) -> Result<lantern_core::Color, StoreError> {
    for &(table, key) in candidates {
        let Ok(literal) = entry(document, path, (table, key)) else {
            continue;
        };

        return base
            .resolve(&literal)
            .map_err(|source| StoreError::ThemeColor {
                path: path.to_owned(),
                key: format!("{table}.{key}"),
                source,
            });
    }

    Err(StoreError::ThemeKeyMissing {
        path: path.to_owned(),
        key: candidates
            .iter()
            .map(|(table, key)| format!("{table}.{key}"))
            .collect::<Vec<_>>()
            .join(" or "),
    })
}
