//! Theme and color palettes for the TUI. Multiple themes can be added; the active theme is chosen via opts (e.g. config `theme` key).
//!
//! **Ownership vs [`crate::layout`]:** this crate owns named [`Palette`]s, appearance, and helpers (`current`, `get`, `adjust_surface_rgb`, etc.). It must not import `layout`—callers in `layout::style` and `render` read palettes from here. “Theme” in user-facing copy means the selectable name; [`Palette`] is the concrete color set.

mod color_utils;
pub mod css;
mod palettes;
pub mod syntect;

use ratatui::style::Color;
use std::cell::RefCell;

pub use color_utils::{
    adjust_surface_rgb, color_rgb_to_hex6, color_to_hex6, color_to_hsl_token, color_to_osc11_hex8,
    color_to_osc11_rgba_payload, darken_rgb, lighten_rgb, rgb_euclidean_sq, rgb_to_hex6,
    rgb_to_hsl_token, rgb_to_osc11_hex8, rgb_to_osc11_rgba_payload, rgb_u8_to_hsl,
};
pub use css::{ThemeCss, tokens_for_theme_name, tokens_from_palette};

pub use palettes::{DEFAULT_COLORS, OBLIVION_INK, theme_ordered_list, theme_selector_entries};
pub use syntect::{CodeThemeKeys, SYNTECT_THEME_KEYS};

/// Match TUI `UiConstants` swatch lighten amounts (kept here so `themes` does not depend on `ui`).
const SWATCH_LIGHTEN: f32 = 0.2;
const SWATCH_LIGHTEN_DARK_ON_LIGHT_POPUP: f32 = 0.38;
const SWATCH_LIGHT_THEME_TEXT: f32 = 0.6;

/// Display color for a theme-selector swatch chip (same rules as the TUI picker).
#[must_use]
pub fn theme_selector_swatch(theme: &Palette, popup_theme: &Palette) -> Color {
    match theme.appearance {
        Appearance::Light => lighten_rgb(theme.swatch, SWATCH_LIGHT_THEME_TEXT),
        Appearance::Dark => {
            let pct = if popup_theme.appearance == Appearance::Light {
                SWATCH_LIGHTEN_DARK_ON_LIGHT_POPUP
            } else {
                SWATCH_LIGHTEN
            };
            adjust_surface_rgb(theme.swatch, pct, theme.appearance)
        }
    }
}

/// HSL token (`"H S% L%"`) for web theme-picker swatches.
#[must_use]
pub fn theme_selector_swatch_token(theme: &Palette, popup_theme: &Palette) -> String {
    color_to_hsl_token(theme_selector_swatch(theme, popup_theme))
        .unwrap_or_else(|| rgb_to_hsl_token(0, 0, 0))
}

/// One row in the theme selector list: a non-selectable section label or a selectable theme.
#[derive(Clone, Copy)]
pub enum SelectorEntry {
    Section(&'static str),
    Item(&'static Palette),
}

/// Whether the theme is predominantly dark or light. Drives [`adjust_surface_rgb`]: dark themes *lighten*
/// auxiliary surfaces (code blocks, stripes); light themes *darken* them so the same `pct` reads as a
/// similar “step” off the page background.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Appearance {
    Dark,
    Light,
}

/// Default theme (Oblivion Ink). Used when opts theme is unset or "default".
pub const DEFAULT_THEME: &Palette = &OBLIVION_INK;

/// When squared RGB distance between page [`Palette::background`] and [`Palette::tab_active_fg`] is at
/// most this, KV section titles (e.g. "General") use [`Palette::tab_active_bg`] as foreground — tab label
/// fg targets the pill, not the body background.
pub const TABLE_SECTION_TITLE_TAB_FG_BG_MAX_DIST_SQ: u32 = 2200;

/// Foreground for metadata / writing table section titles; see [`TABLE_SECTION_TITLE_TAB_FG_BG_MAX_DIST_SQ`].
#[must_use]
pub fn table_section_title_fg(palette: &Palette) -> Color {
    match color_utils::rgb_euclidean_sq(palette.background, palette.tab_active_fg) {
        Some(d) if d <= TABLE_SECTION_TITLE_TAB_FG_BG_MAX_DIST_SQ => palette.tab_active_bg,
        _ => palette.text,
    }
}

/// Display name written to new global/local default `ublx.toml` files ([`DEFAULT_THEME`]).
#[inline]
#[must_use]
pub fn default_theme_for_new_config_file() -> &'static str {
    DEFAULT_THEME.name
}

/// Named set of colors for the TUI. Extend with more fields as styles are themed.
#[derive(Clone, Debug)]
pub struct Palette {
    // #[allow(dead_code)]
    pub name: &'static str,
    /// See [`Appearance`] and [`adjust_surface_rgb`].
    pub appearance: Appearance,
    /// Background for the whole app (full frame).
    pub background: Color,
    /// Default foreground for body text (list items, paragraphs, search input).
    pub text: Color,
    /// Focused panel border (e.g. categories/contents).
    pub focused_border: Color,
    /// Active tab foreground.
    pub tab_active_fg: Color,
    /// Active tab background.
    pub tab_active_bg: Color,
    /// Inactive tab background.
    pub tab_inactive_bg: Color,
    /// "Search:" label text in status line.
    pub search_text: Color,
    /// Hint text (e.g. "Esc to clear").
    pub hint: Color,
    /// Popup background (help box, theme selector, etc.)
    pub popup_bg: Color,
    /// Node/footer powerline fill on **dark** themes. On **light** themes, footers use [`node_pill_background`] instead so pills don’t match the page.
    pub node_bg: Color,
    /// Toast/notification overlay background
    pub notification_bg: Color,
    /// Delta: added (green)
    pub delta_added: Color,
    /// Delta: modified (yellow)
    pub delta_mod: Color,
    /// Delta: removed (red)
    pub delta_removed: Color,
    /// Brand title (e.g. UBLX)
    pub title_brand: Color,
    /// Color for theme selector swatch
    pub swatch: Color,
}

// NOTE: Intentionally no `Theme` type alias. "theme" is a user-facing concept (select by name);
// `Palette` is the underlying color set type.

/// HSL step off [`Palette::background`] for light-theme footer/status powerline pills.
const LIGHT_THEME_NODE_PILL_PCT: f32 = 0.11;

/// Fill color for powerline footer nodes and related chrome. Dark themes: [`Palette::node_bg`]. Light themes: [`adjust_surface_rgb`] from background so bars read against the page.
#[must_use]
pub fn node_pill_background(theme: &Palette) -> Color {
    match theme.appearance {
        Appearance::Light => adjust_surface_rgb(
            theme.background,
            LIGHT_THEME_NODE_PILL_PCT,
            theme.appearance,
        ),
        Appearance::Dark => theme.node_bg,
    }
}

thread_local! {
    static CURRENT_THEME_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Set the theme name for the current frame (called at start of draw). Style functions use this.
pub fn set_current(name: Option<&str>) {
    CURRENT_THEME_NAME.with(|cell| {
        *cell.borrow_mut() = name.map(String::from);
    });
}

/// Current theme for this frame. Use in style functions.
#[must_use]
pub fn current() -> &'static Palette {
    let name = CURRENT_THEME_NAME.with(|cell| cell.borrow().clone());
    get(name.as_deref())
}

/// Resolve config theme to the theme name to use. When config is `None`, empty, or `"default"`, returns [`DEFAULT_THEME`]'s name. Otherwise returns the config value. Use before passing to [`set_current`] or [`get`].
#[must_use]
pub fn theme_name_from_config(config_theme: Option<&str>) -> &str {
    match config_theme {
        None => DEFAULT_THEME.name,
        Some(s) => {
            let t = s.trim();
            if t.is_empty() || t == "default" {
                DEFAULT_THEME.name
            } else {
                t
            }
        }
    }
}

/// Resolve theme by name. Uses [`theme_name_from_config`] so `None` / empty / `"default"` use [`DEFAULT_THEME`]; then matches [`Palette::name`] (same strings as in [`theme_ordered_list`] / TOML).
#[must_use]
pub fn get(name: Option<&str>) -> &'static Palette {
    let n = theme_name_from_config(name);
    if n == DEFAULT_THEME.name {
        return DEFAULT_THEME;
    }
    theme_ordered_list()
        .iter()
        .copied()
        .find(|t| t.name == n)
        .unwrap_or(DEFAULT_THEME)
}
