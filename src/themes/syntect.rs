//! Keys into syntect’s embedded [`syntect::highlighting::ThemeSet::load_defaults`] for the viewer.
//!
//! Each [`Palette`](crate::themes::Palette) sets [`Palette::syntect`] to one of these names.
//! Unknown keys fall back to [`SYNTECT_THEME_KEYS`] by [`Appearance`](crate::themes::Appearance).
//!
//! Built-in defaults (syntect 5): `InspiredGitHub`, `Solarized (dark)`, `Solarized (light)`,
//! `base16-eighties.dark`, `base16-mocha.dark`, `base16-ocean.dark`, `base16-ocean.light`.

use std::sync::LazyLock;

use syntect::highlighting::{Theme, ThemeSet};

use super::{Appearance, Palette};

/// Canonical names from `ThemeSet::load_defaults()` — keep in sync with syntect.
pub const SYNTECT_DEFAULT_KEYS: &[&str] = &[
    "InspiredGitHub",
    "Solarized (dark)",
    "Solarized (light)",
    "base16-eighties.dark",
    "base16-mocha.dark",
    "base16-ocean.dark",
    "base16-ocean.light",
];

/// String keys for the built-in themedump (`base16-ocean.dark`, `InspiredGitHub`, …).
#[derive(Clone, Copy, Debug)]
pub struct CodeThemeKeys {
    pub dark: &'static str,
    pub light: &'static str,
    /// If `dark` / `light` is missing from the set, use this (must exist in syntect defaults).
    pub fallback: &'static str,
}

/// Appearance fallbacks when a palette’s [`Palette::syntect`] key is unknown.
pub const SYNTECT_THEME_KEYS: CodeThemeKeys = CodeThemeKeys {
    dark: "base16-ocean.dark",
    light: "InspiredGitHub",
    fallback: "base16-ocean.dark",
};

static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

fn appearance_fallback_key(appearance: Appearance) -> &'static str {
    let k = &SYNTECT_THEME_KEYS;
    match appearance {
        Appearance::Dark => k.dark,
        Appearance::Light => k.light,
    }
}

/// Resolve a syntect theme key: prefer `key` if known, else appearance fallback.
#[must_use]
pub fn resolve_syntect_key(key: &str, appearance: Appearance) -> &'static str {
    SYNTECT_DEFAULT_KEYS
        .iter()
        .copied()
        .find(|&k| k == key)
        .unwrap_or_else(|| appearance_fallback_key(appearance))
}

/// Syntect theme for a UBLX [`Palette`].
#[must_use]
pub fn theme_for_palette(palette: &Palette) -> &'static Theme {
    theme_for_key(palette.syntect, palette.appearance)
}

/// Look up a syntect theme by key with appearance fallback.
#[must_use]
pub fn theme_for_key(key: &str, appearance: Appearance) -> &'static Theme {
    let resolved = resolve_syntect_key(key, appearance);
    THEME_SET.themes.get(resolved).unwrap_or_else(|| {
        THEME_SET
            .themes
            .get(SYNTECT_THEME_KEYS.fallback)
            .expect("syntect fallback theme")
    })
}
