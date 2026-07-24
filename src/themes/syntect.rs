//! Syntect themes for the code Viewer.
//!
//! Each UBLX [`Palette`](crate::themes::Palette) has a matching first-pass `.tmTheme` under
//! `assets/syntect-themes/` (colors derived from that palette). Syntect defaults remain as
//! appearance fallbacks when a key is missing.

use std::io::Cursor;
use std::sync::LazyLock;

use syntect::highlighting::{Theme, ThemeSet};

use super::{Appearance, Palette};

/// Built-in keys from `ThemeSet::load_defaults()` (appearance fallbacks only).
pub const SYNTECT_DEFAULT_KEYS: &[&str] = &[
    "InspiredGitHub",
    "Solarized (dark)",
    "Solarized (light)",
    "base16-eighties.dark",
    "base16-mocha.dark",
    "base16-ocean.dark",
    "base16-ocean.light",
];

/// UBLX-authored theme keys (`assets/syntect-themes/<key>.tmTheme`).
pub const SYNTECT_EXTRA_KEYS: &[&str] = &[
    "shadow-index",
    "archival-simulacra",
    "oblivion-ink",
    "garden-unseen",
    "burning-glyph",
    "golden-delirium",
    "tangerine-memory",
    "purple-haze",
    "frozen-phrase",
    "babel-blend",
    "resin-record",
    "silent-sheet",
    "obdurate-noon",
    "faded-echo",
    "parched-page",
    "pale-mirror",
    "ochre-thread",
    "cryptic-chai",
    "asterion-code",
    "infinite-rose",
    "barley-bound",
    "verglas-trace",
];

/// String keys for appearance fallbacks when a palette’s [`Palette::syntect`] is missing.
#[derive(Clone, Copy, Debug)]
pub struct CodeThemeKeys {
    pub dark: &'static str,
    pub light: &'static str,
    pub fallback: &'static str,
}

/// Appearance fallbacks (prefer our Oblivion / Silent Sheet packs).
pub const SYNTECT_THEME_KEYS: CodeThemeKeys = CodeThemeKeys {
    dark: "oblivion-ink",
    light: "silent-sheet",
    fallback: "oblivion-ink",
};

static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(load_theme_set);

fn load_theme_set() -> ThemeSet {
    let mut set = ThemeSet::load_defaults();
    for (key, bytes) in EMBEDDED_THEMES {
        let mut reader = Cursor::new(bytes);
        match ThemeSet::load_from_reader(&mut reader) {
            Ok(theme) => {
                set.themes.insert((*key).to_owned(), theme);
            }
            Err(e) => {
                eprintln!("ublx: failed to load embedded syntect theme {key:?}: {e}");
            }
        }
    }
    set
}

const EMBEDDED_THEMES: &[(&str, &[u8])] = &[
    (
        "shadow-index",
        include_bytes!("../../assets/syntect-themes/shadow-index.tmTheme"),
    ),
    (
        "archival-simulacra",
        include_bytes!("../../assets/syntect-themes/archival-simulacra.tmTheme"),
    ),
    (
        "oblivion-ink",
        include_bytes!("../../assets/syntect-themes/oblivion-ink.tmTheme"),
    ),
    (
        "garden-unseen",
        include_bytes!("../../assets/syntect-themes/garden-unseen.tmTheme"),
    ),
    (
        "burning-glyph",
        include_bytes!("../../assets/syntect-themes/burning-glyph.tmTheme"),
    ),
    (
        "golden-delirium",
        include_bytes!("../../assets/syntect-themes/golden-delirium.tmTheme"),
    ),
    (
        "tangerine-memory",
        include_bytes!("../../assets/syntect-themes/tangerine-memory.tmTheme"),
    ),
    (
        "purple-haze",
        include_bytes!("../../assets/syntect-themes/purple-haze.tmTheme"),
    ),
    (
        "frozen-phrase",
        include_bytes!("../../assets/syntect-themes/frozen-phrase.tmTheme"),
    ),
    (
        "babel-blend",
        include_bytes!("../../assets/syntect-themes/babel-blend.tmTheme"),
    ),
    (
        "resin-record",
        include_bytes!("../../assets/syntect-themes/resin-record.tmTheme"),
    ),
    (
        "silent-sheet",
        include_bytes!("../../assets/syntect-themes/silent-sheet.tmTheme"),
    ),
    (
        "obdurate-noon",
        include_bytes!("../../assets/syntect-themes/obdurate-noon.tmTheme"),
    ),
    (
        "faded-echo",
        include_bytes!("../../assets/syntect-themes/faded-echo.tmTheme"),
    ),
    (
        "parched-page",
        include_bytes!("../../assets/syntect-themes/parched-page.tmTheme"),
    ),
    (
        "pale-mirror",
        include_bytes!("../../assets/syntect-themes/pale-mirror.tmTheme"),
    ),
    (
        "ochre-thread",
        include_bytes!("../../assets/syntect-themes/ochre-thread.tmTheme"),
    ),
    (
        "cryptic-chai",
        include_bytes!("../../assets/syntect-themes/cryptic-chai.tmTheme"),
    ),
    (
        "asterion-code",
        include_bytes!("../../assets/syntect-themes/asterion-code.tmTheme"),
    ),
    (
        "infinite-rose",
        include_bytes!("../../assets/syntect-themes/infinite-rose.tmTheme"),
    ),
    (
        "barley-bound",
        include_bytes!("../../assets/syntect-themes/barley-bound.tmTheme"),
    ),
    (
        "verglas-trace",
        include_bytes!("../../assets/syntect-themes/verglas-trace.tmTheme"),
    ),
];

fn appearance_fallback_key(appearance: Appearance) -> &'static str {
    let k = &SYNTECT_THEME_KEYS;
    match appearance {
        Appearance::Dark => k.dark,
        Appearance::Light => k.light,
    }
}

/// Resolve a syntect theme key: prefer `key` if loaded, else appearance fallback.
#[must_use]
pub fn resolve_syntect_key(key: &str, appearance: Appearance) -> &'static str {
    if THEME_SET.themes.contains_key(key) {
        SYNTECT_EXTRA_KEYS
            .iter()
            .chain(SYNTECT_DEFAULT_KEYS.iter())
            .copied()
            .find(|&k| k == key)
            .unwrap_or_else(|| appearance_fallback_key(appearance))
    } else {
        appearance_fallback_key(appearance)
    }
}

/// Syntect theme for a UBLX [`Palette`].
#[must_use]
pub fn theme_for_palette(palette: &Palette) -> &'static Theme {
    theme_for_key(palette.syntect, palette.appearance)
}

/// Look up a syntect theme by key with appearance fallback.
#[must_use]
pub fn theme_for_key(key: &str, appearance: Appearance) -> &'static Theme {
    if let Some(t) = THEME_SET.themes.get(key) {
        return t;
    }
    let resolved = appearance_fallback_key(appearance);
    THEME_SET.themes.get(resolved).unwrap_or_else(|| {
        THEME_SET
            .themes
            .get(SYNTECT_THEME_KEYS.fallback)
            .expect("syntect fallback theme")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::palettes::theme_ordered_list;

    #[test]
    fn embedded_extra_themes_load() {
        for key in SYNTECT_EXTRA_KEYS {
            assert!(
                THEME_SET.themes.contains_key(*key),
                "missing embedded syntect theme {key}"
            );
        }
    }

    #[test]
    fn every_palette_syntect_key_resolves() {
        for p in theme_ordered_list() {
            assert!(
                THEME_SET.themes.contains_key(p.syntect),
                "palette {} references unknown syntect key {}",
                p.name,
                p.syntect
            );
            let _ = theme_for_palette(p);
        }
    }
}
