//! Map [`Palette`] → CSS custom properties (`"H S% L%"` tokens) for the web UI.
//!
//! **Scaffold is the TUI.** Token *names* follow shadcn for leptos-shadcn-ui, but **which
//! fields paint together** must match [`crate::layout::style::ThemeStyles`] (e.g. active tabs
//! use `tab_active_fg` on `tab_active_bg`, never `focused_border` on `tab_active_bg`).
//! See `docs/WEB_UI.md` § Theming.

use std::collections::BTreeMap;

use ratatui::style::Color;
use serde::Serialize;

use super::color_utils::{color_to_hsl_token, rgb_to_hsl_token};
use super::{Appearance, Palette, node_pill_background};

/// Live theme payload for the web UI (`GET`/`PATCH /settings/{scope}`).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ThemeCss {
    /// Effective palette name (merged global+local).
    pub name: String,
    /// `"dark"` or `"light"` — toggles `html.dark` on the web client.
    pub appearance: String,
    /// CSS custom properties keyed with leading `--` (e.g. `--background` → `"214 65% 11%"`).
    pub vars: BTreeMap<String, String>,
}

fn token(color: Color) -> String {
    color_to_hsl_token(color).unwrap_or_else(|| rgb_to_hsl_token(0, 0, 0))
}

fn insert(vars: &mut BTreeMap<String, String>, name: &str, color: Color) {
    vars.insert(name.to_string(), token(color));
}

/// Build CSS tokens from a [`Palette`] using the WEB_UI theming map.
#[must_use]
pub fn tokens_from_palette(palette: &Palette) -> ThemeCss {
    let mut vars = BTreeMap::new();

    let bg = palette.background;
    let fg = palette.text;
    let focus = palette.focused_border;
    let pill = node_pill_background(palette);
    let inverse_fg = match palette.appearance {
        Appearance::Dark => Color::Rgb(255, 255, 255),
        Appearance::Light => Color::Rgb(0, 0, 0),
    };

    // Core shadcn tokens
    insert(&mut vars, "--background", bg);
    insert(&mut vars, "--foreground", fg);
    insert(&mut vars, "--card", palette.popup_bg);
    insert(&mut vars, "--card-foreground", fg);
    insert(&mut vars, "--popover", palette.popup_bg);
    insert(&mut vars, "--popover-foreground", fg);
    insert(&mut vars, "--primary", focus);
    insert(&mut vars, "--primary-foreground", bg);
    insert(&mut vars, "--secondary", palette.tab_active_bg);
    insert(&mut vars, "--secondary-foreground", palette.tab_active_fg);
    insert(&mut vars, "--muted", palette.tab_inactive_bg);
    insert(&mut vars, "--muted-foreground", palette.hint);
    insert(&mut vars, "--accent", palette.popup_bg);
    insert(&mut vars, "--accent-foreground", fg);
    insert(&mut vars, "--destructive", palette.delta_removed);
    insert(&mut vars, "--destructive-foreground", inverse_fg);
    insert(&mut vars, "--border", pill);
    insert(&mut vars, "--input", pill);
    insert(&mut vars, "--ring", focus);

    // TUI-specific extras (WEB_UI token table)
    insert(&mut vars, "--brand", palette.title_brand);
    insert(&mut vars, "--hint", palette.hint);
    insert(&mut vars, "--search", palette.search_text);
    insert(&mut vars, "--delta-added", palette.delta_added);
    insert(&mut vars, "--delta-mod", palette.delta_mod);
    insert(&mut vars, "--delta-removed", palette.delta_removed);
    insert(&mut vars, "--node", pill);

    ThemeCss {
        name: palette.name.to_string(),
        appearance: match palette.appearance {
            Appearance::Dark => "dark".into(),
            Appearance::Light => "light".into(),
        },
        vars,
    }
}

/// Resolve theme by name (same rules as [`super::get`]) and build CSS tokens.
#[must_use]
pub fn tokens_for_theme_name(name: Option<&str>) -> ThemeCss {
    tokens_from_palette(super::get(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::OBLIVION_INK;

    #[test]
    fn oblivion_ink_maps_core_roles() {
        let css = tokens_from_palette(&OBLIVION_INK);
        assert_eq!(css.name, "Oblivion Ink");
        assert_eq!(css.appearance, "dark");
        assert!(css.vars.contains_key("--background"));
        assert!(css.vars.contains_key("--foreground"));
        assert!(css.vars.contains_key("--ring"));
        assert!(css.vars.contains_key("--brand"));
        assert!(css.vars.contains_key("--search"));
        assert!(css.vars.contains_key("--delta-added"));
        // Oblivion Ink page ≈ 214 65% 11%
        let bg = css.vars.get("--background").expect("bg");
        assert!(bg.starts_with("21"), "unexpected bg token {bg}");
        assert!(bg.contains('%'), "expected HSL percents in {bg}");
    }
}
