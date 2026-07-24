//! Built-in [`Palette`] values for the TUI.
//!
//! [`Palette::name`] is the exact string for config `theme = "..."` and the theme picker. New themes
//! must be appended to the `ALL_THEMES` array at the end of this file so they appear in
//! [`theme_ordered_list`] and [`theme_selector_entries`].

use std::sync::LazyLock;

use ratatui::style::Color;

use super::{Appearance, Palette, SelectorEntry};

/// Named channels for [`DEFAULT_COLORS`] (not every field is used by every style path).
pub struct DefaultColors {
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub cyan: Color,
    pub magenta: Color,
    pub gray: Color,
    pub black: Color,
    pub white: Color,
}

/// Default delta row colors (added / modified / removed). Every built-in palette uses these for
/// [`Palette::delta_added`], [`Palette::delta_mod`], [`Palette::delta_removed`].
pub const DEFAULT_COLORS: DefaultColors = DefaultColors {
    green: Color::Rgb(72, 187, 120),
    yellow: Color::Rgb(253, 203, 110),
    red: Color::Rgb(239, 68, 68),
    cyan: Color::Rgb(42, 161, 152),
    magenta: Color::Rgb(164, 95, 250),
    gray: Color::Rgb(128, 128, 128),
    black: Color::Rgb(0, 0, 0),
    white: Color::Rgb(255, 255, 255),
};

// ---- Shadow Index ----
// Dark: near-black page, cool off-white text, gray focus (not a light/white background).
pub static SHADOW_INDEX: Palette = Palette {
    name: "Shadow Index",
    appearance: Appearance::Dark,
    syntect: "shadow-index",
    background: Color::Rgb(0, 0, 0),
    text: Color::Rgb(235, 236, 242),
    focused_border: Color::Rgb(153, 153, 153),
    tab_active_fg: Color::Rgb(235, 236, 242),
    tab_active_bg: Color::Rgb(48, 52, 68),
    tab_inactive_bg: Color::Rgb(4, 4, 6),
    search_text: Color::Rgb(128, 128, 128),
    hint: Color::Rgb(128, 128, 153),
    popup_bg: Color::Rgb(13, 12, 12),
    node_bg: Color::Rgb(27, 24, 24),
    notification_bg: Color::Rgb(13, 12, 12),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(128, 128, 153),
    swatch: Color::Rgb(0, 0, 0),
};

// ---- Archival Simulacra ----
// Dark: true black, neon green body and tab chrome.
pub static ARCHIVAL_SIMULACRA: Palette = Palette {
    name: "Archival Simulacra",
    appearance: Appearance::Dark,
    syntect: "archival-simulacra",
    background: Color::Rgb(0, 0, 0),
    text: Color::Rgb(68, 255, 106),
    focused_border: Color::Rgb(140, 255, 120),
    tab_active_fg: Color::Rgb(0, 0, 0),
    tab_active_bg: Color::Rgb(72, 255, 112),
    tab_inactive_bg: Color::Rgb(0, 14, 4),
    search_text: Color::Rgb(48, 220, 88),
    hint: Color::Rgb(0, 140, 54),
    popup_bg: Color::Rgb(0, 10, 3),
    node_bg: Color::Rgb(0, 20, 7),
    notification_bg: Color::Rgb(0, 10, 3),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(110, 255, 130),
    swatch: Color::Rgb(68, 255, 106),
};

// ---- Oblivion Ink ----
// Default dark theme ([`crate::themes::DEFAULT_THEME`]).
pub static OBLIVION_INK: Palette = Palette {
    name: "Oblivion Ink",
    appearance: Appearance::Dark,
    syntect: "oblivion-ink",
    background: Color::Rgb(10, 25, 47),
    text: Color::Rgb(200, 235, 230),
    focused_border: Color::Rgb(100, 255, 218),
    tab_active_fg: Color::Rgb(100, 255, 218),
    tab_active_bg: Color::Rgb(22, 58, 92),
    tab_inactive_bg: Color::Rgb(9, 22, 42),
    search_text: Color::Rgb(100, 255, 218),
    hint: Color::Rgb(164, 95, 250),
    popup_bg: Color::Rgb(14, 35, 66),
    node_bg: Color::Rgb(17, 45, 85),
    notification_bg: Color::Rgb(14, 35, 66),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(164, 95, 250),
    swatch: Color::Rgb(10, 25, 47),
};

// ---- Garden Unseen ----
// Dark: deep forest green page, warm peach text, mint accents.
pub static GARDEN_UNSEEN: Palette = Palette {
    name: "Garden Unseen",
    appearance: Appearance::Dark,
    syntect: "garden-unseen",
    background: Color::Rgb(0, 42, 21),
    text: Color::Rgb(194, 150, 132),
    focused_border: Color::Rgb(130, 246, 198),
    tab_active_fg: Color::Rgb(230, 206, 182),
    tab_active_bg: Color::Rgb(24, 92, 75),
    tab_inactive_bg: Color::Rgb(0, 40, 22),
    search_text: Color::Rgb(98, 237, 181),
    hint: Color::Rgb(102, 94, 70),
    popup_bg: Color::Rgb(0, 65, 33),
    node_bg: Color::Rgb(0, 89, 44),
    notification_bg: Color::Rgb(0, 65, 33),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(73, 67, 44),
    swatch: Color::Rgb(0, 42, 21),
};

// ---- Burning Glyph ----
// Dark: maroon-black page, warm text, red/coral focus (accent family: red).
pub static BURNING_GLYPH: Palette = Palette {
    name: "Burning Glyph",
    appearance: Appearance::Dark,
    syntect: "burning-glyph",
    background: Color::Rgb(42, 0, 0),
    text: Color::Rgb(249, 233, 170),
    focused_border: Color::Rgb(246, 130, 130),
    tab_active_fg: Color::Rgb(210, 200, 238),
    tab_active_bg: Color::Rgb(82, 44, 18),
    tab_inactive_bg: Color::Rgb(38, 0, 0),
    search_text: Color::Rgb(237, 98, 98),
    hint: Color::Rgb(237, 181, 98),
    popup_bg: Color::Rgb(65, 0, 0),
    node_bg: Color::Rgb(89, 0, 0),
    notification_bg: Color::Rgb(65, 0, 0),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(237, 181, 98),
    swatch: Color::Rgb(42, 0, 0),
};

// ---- Golden Delirium ----
// Dark: olive-black page, soft pink/cream text, yellow-lime focus (accent family: yellow).
pub static GOLDEN_DELIRIUM: Palette = Palette {
    name: "Golden Delirium",
    appearance: Appearance::Dark,
    syntect: "golden-delirium",
    background: Color::Rgb(42, 42, 0),
    text: Color::Rgb(253, 196, 199),
    focused_border: Color::Rgb(246, 246, 130),
    tab_active_fg: Color::Rgb(252, 246, 188),
    tab_active_bg: Color::Rgb(36, 64, 40),
    tab_inactive_bg: Color::Rgb(38, 38, 0),
    search_text: Color::Rgb(237, 237, 98),
    hint: Color::Rgb(167, 107, 78),
    popup_bg: Color::Rgb(65, 65, 0),
    node_bg: Color::Rgb(89, 89, 0),
    notification_bg: Color::Rgb(65, 65, 0),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(167, 107, 78),
    swatch: Color::Rgb(42, 42, 0),
};

// ---- Tangerine Memory ----
// Dark: burnt umber page, honey-cream text, peach-gold focus (accent family: orange).
pub static TANGERINE_MEMORY: Palette = Palette {
    name: "Tangerine Memory",
    appearance: Appearance::Dark,
    syntect: "tangerine-memory",
    background: Color::Rgb(42, 26, 0),
    text: Color::Rgb(246, 163, 142),
    focused_border: Color::Rgb(246, 198, 130),
    tab_active_fg: Color::Rgb(255, 226, 205),
    tab_active_bg: Color::Rgb(72, 38, 52),
    tab_inactive_bg: Color::Rgb(40, 24, 0),
    search_text: Color::Rgb(237, 155, 98),
    hint: Color::Rgb(226, 169, 157),
    popup_bg: Color::Rgb(65, 40, 0),
    node_bg: Color::Rgb(89, 55, 0),
    notification_bg: Color::Rgb(65, 40, 0),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(226, 169, 157),
    swatch: Color::Rgb(42, 26, 0),
};

// ---- Purple Haze ----
// Dark: near-black violet page, magenta/pink focus (accent family: purple).
pub static PURPLE_HAZE: Palette = Palette {
    name: "Purple Haze",
    appearance: Appearance::Dark,
    syntect: "purple-haze",
    background: Color::Rgb(13, 0, 26),
    text: Color::Rgb(205, 114, 125),
    focused_border: Color::Rgb(241, 130, 246),
    tab_active_fg: Color::Rgb(234, 220, 255),
    tab_active_bg: Color::Rgb(58, 18, 118),
    tab_inactive_bg: Color::Rgb(12, 0, 24),
    search_text: Color::Rgb(221, 98, 237),
    hint: Color::Rgb(116, 80, 240),
    popup_bg: Color::Rgb(25, 0, 50),
    node_bg: Color::Rgb(37, 0, 74),
    notification_bg: Color::Rgb(25, 0, 50),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(116, 80, 240),
    swatch: Color::Rgb(13, 0, 26),
};

// ---- Frozen Phrase (dark: Nordic polar night, frost blue chrome) ----
pub static FROZEN_PHRASE: Palette = Palette {
    name: "Frozen Phrase",
    appearance: Appearance::Dark,
    syntect: "frozen-phrase",
    background: Color::Rgb(46, 52, 64),
    text: Color::Rgb(216, 232, 252),
    focused_border: Color::Rgb(129, 178, 208),
    tab_active_fg: Color::Rgb(36, 42, 54),
    tab_active_bg: Color::Rgb(206, 216, 233),
    tab_inactive_bg: Color::Rgb(38, 44, 56),
    search_text: Color::Rgb(143, 188, 188),
    hint: Color::Rgb(172, 186, 207),
    popup_bg: Color::Rgb(40, 48, 62),
    node_bg: Color::Rgb(54, 64, 82),
    notification_bg: Color::Rgb(40, 48, 62),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(180, 200, 224),
    swatch: Color::Rgb(46, 52, 64),
};

// ---- Babel Blend (dark: deep navy page; orange focus/search vs red tab chrome — hues separated by role) ----
pub static BABEL_BLEND: Palette = Palette {
    name: "Babel Blend",
    appearance: Appearance::Dark,
    syntect: "babel-blend",
    background: Color::Rgb(12, 22, 46),
    text: Color::Rgb(236, 172, 158),
    focused_border: Color::Rgb(255, 145, 62),
    tab_active_fg: Color::Rgb(255, 252, 248),
    tab_active_bg: Color::Rgb(168, 42, 48),
    tab_inactive_bg: Color::Rgb(18, 30, 58),
    search_text: Color::Rgb(255, 178, 92),
    hint: Color::Rgb(232, 96, 88),
    popup_bg: Color::Rgb(16, 28, 54),
    node_bg: Color::Rgb(22, 36, 68),
    notification_bg: Color::Rgb(16, 28, 54),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(255, 118, 58),
    swatch: Color::Rgb(12, 22, 46),
};

// ---- Resin Record (dark: amber-terminal phosphor on near-black field) ----
pub static RESIN_RECORD: Palette = Palette {
    name: "Resin Record",
    appearance: Appearance::Dark,
    syntect: "resin-record",
    background: Color::Rgb(18, 12, 0),
    text: Color::Rgb(248, 176, 142),
    focused_border: Color::Rgb(230, 170, 60),
    tab_active_fg: Color::Rgb(20, 12, 0),
    tab_active_bg: Color::Rgb(245, 176, 74),
    tab_inactive_bg: Color::Rgb(34, 22, 5),
    search_text: Color::Rgb(255, 190, 92),
    hint: Color::Rgb(168, 120, 58),
    popup_bg: Color::Rgb(24, 16, 2),
    node_bg: Color::Rgb(36, 24, 4),
    notification_bg: Color::Rgb(24, 16, 2),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(214, 140, 52),
    swatch: Color::Rgb(214, 140, 52),
};

// ---- Silent Sheet ----
// Light: white page, black text, slate active tabs (minimal monochrome).
pub static SILENT_SHEET: Palette = Palette {
    name: "Silent Sheet",
    appearance: Appearance::Light,
    syntect: "silent-sheet",
    background: Color::Rgb(255, 255, 255),
    text: Color::Rgb(0, 0, 0),
    focused_border: Color::Rgb(77, 77, 77),
    tab_active_fg: Color::Rgb(255, 255, 255),
    tab_active_bg: Color::Rgb(58, 68, 92),
    tab_inactive_bg: Color::Rgb(242, 242, 244),
    search_text: Color::Rgb(102, 102, 102),
    hint: Color::Rgb(102, 92, 82),
    popup_bg: Color::Rgb(255, 255, 255),
    node_bg: Color::Rgb(255, 255, 255),
    notification_bg: Color::Rgb(255, 255, 255),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(102, 92, 82),
    swatch: Color::Rgb(0, 0, 0),
};

// ---- Obdurate Noon (light: Solarized-light inspired parchment with cyan/blue accents) ----
pub static OBDURATE_NOON: Palette = Palette {
    name: "Obdurate Noon",
    appearance: Appearance::Light,
    syntect: "obdurate-noon",
    background: Color::Rgb(253, 246, 227),      // base3
    text: Color::Rgb(101, 123, 131),            // base00
    focused_border: Color::Rgb(42, 161, 152),   // cyan
    tab_active_fg: Color::Rgb(253, 246, 227),   // base3
    tab_active_bg: Color::Rgb(38, 139, 210),    // blue
    tab_inactive_bg: Color::Rgb(238, 232, 213), // base2
    search_text: Color::Rgb(38, 139, 210),      // blue
    hint: Color::Rgb(108, 113, 196),            // violet
    popup_bg: Color::Rgb(247, 240, 219),
    node_bg: Color::Rgb(244, 236, 212),
    notification_bg: Color::Rgb(246, 238, 216),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(181, 137, 0), // yellow
    swatch: Color::Rgb(101, 123, 131),
};

// ---- Faded Echo (light: dusty sepia paper and book-ink) ----
pub static FADED_ECHO: Palette = Palette {
    name: "Faded Echo",
    appearance: Appearance::Light,
    syntect: "faded-echo",
    background: Color::Rgb(244, 236, 222),
    text: Color::Rgb(72, 62, 50),
    focused_border: Color::Rgb(126, 108, 86),
    tab_active_fg: Color::Rgb(255, 249, 238),
    tab_active_bg: Color::Rgb(98, 82, 64),
    tab_inactive_bg: Color::Rgb(240, 229, 208),
    search_text: Color::Rgb(118, 98, 80),
    hint: Color::Rgb(112, 106, 96),
    popup_bg: Color::Rgb(238, 226, 203),
    node_bg: Color::Rgb(235, 223, 199),
    notification_bg: Color::Rgb(237, 225, 201),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(156, 104, 58),
    swatch: Color::Rgb(78, 61, 44),
};

// ---- Parched Page (light: dry manuscript paper, ink-green text) ----
pub static PARCHED_PAGE: Palette = Palette {
    name: "Parched Page",
    appearance: Appearance::Light,
    syntect: "parched-page",
    background: Color::Rgb(253, 248, 240),
    text: Color::Rgb(24, 78, 52),
    focused_border: Color::Rgb(52, 130, 88),
    tab_active_fg: Color::Rgb(255, 255, 255),
    tab_active_bg: Color::Rgb(38, 92, 64),
    tab_inactive_bg: Color::Rgb(251, 246, 237),
    search_text: Color::Rgb(72, 115, 88),
    hint: Color::Rgb(130, 98, 62),
    popup_bg: Color::Rgb(253, 244, 231),
    node_bg: Color::Rgb(253, 245, 233),
    notification_bg: Color::Rgb(252, 243, 229),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(48, 105, 72),
    swatch: Color::Rgb(24, 78, 52),
};

// ---- Pale Mirror (light: frosted blue-lilac page, plum ink — no green/teal chrome) ----
pub static PALE_MIRROR: Palette = Palette {
    name: "Pale Mirror",
    appearance: Appearance::Light,
    syntect: "pale-mirror",
    background: Color::Rgb(242, 245, 253),
    text: Color::Rgb(42, 30, 56),
    focused_border: Color::Rgb(118, 78, 152),
    tab_active_fg: Color::Rgb(255, 255, 255),
    tab_active_bg: Color::Rgb(74, 48, 102),
    tab_inactive_bg: Color::Rgb(244, 246, 252),
    search_text: Color::Rgb(108, 92, 132),
    hint: Color::Rgb(138, 88, 118),
    popup_bg: Color::Rgb(235, 238, 248),
    node_bg: Color::Rgb(233, 236, 246),
    notification_bg: Color::Rgb(234, 237, 247),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(96, 68, 128),
    swatch: Color::Rgb(42, 30, 56),
};

// ---- Ochre Thread (light: pale sand page, rust ink) ----
pub static OCHRE_THREAD: Palette = Palette {
    name: "Ochre Thread",
    appearance: Appearance::Light,
    syntect: "ochre-thread",
    background: Color::Rgb(250, 242, 228),
    text: Color::Rgb(110, 50, 22),
    focused_border: Color::Rgb(178, 88, 34),
    tab_active_fg: Color::Rgb(255, 252, 248),
    tab_active_bg: Color::Rgb(92, 44, 24),
    tab_inactive_bg: Color::Rgb(246, 237, 220),
    search_text: Color::Rgb(148, 96, 68),
    hint: Color::Rgb(88, 102, 118),
    popup_bg: Color::Rgb(242, 232, 216),
    node_bg: Color::Rgb(240, 230, 214),
    notification_bg: Color::Rgb(241, 231, 215),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(162, 74, 30),
    swatch: Color::Rgb(110, 50, 22),
};

// ---- Cryptic Chai (light: tea-stained parchment, chocolate ink) ----
pub static CRYPTIC_CHAI: Palette = Palette {
    name: "Cryptic Chai",
    appearance: Appearance::Light,
    syntect: "cryptic-chai",
    background: Color::Rgb(247, 238, 222),
    text: Color::Rgb(58, 38, 28),
    focused_border: Color::Rgb(152, 92, 48),
    tab_active_fg: Color::Rgb(255, 250, 244),
    tab_active_bg: Color::Rgb(78, 48, 36),
    tab_inactive_bg: Color::Rgb(244, 234, 218),
    search_text: Color::Rgb(124, 88, 68),
    hint: Color::Rgb(108, 98, 72),
    popup_bg: Color::Rgb(239, 228, 208),
    node_bg: Color::Rgb(237, 226, 205),
    notification_bg: Color::Rgb(238, 227, 206),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(138, 72, 42),
    swatch: Color::Rgb(58, 38, 28),
};

// ---- Asterion Code (light: cool blue-gray stone, blue-forward ink — not Parched’s warm cream + forest green) ----
pub static ASTERION_CODE: Palette = Palette {
    name: "Asterion Code",
    appearance: Appearance::Light,
    syntect: "asterion-code",
    background: Color::Rgb(232, 240, 242),
    text: Color::Rgb(10, 72, 96),
    focused_border: Color::Rgb(26, 128, 168),
    tab_active_fg: Color::Rgb(248, 252, 255),
    tab_active_bg: Color::Rgb(16, 82, 108),
    tab_inactive_bg: Color::Rgb(226, 234, 237),
    search_text: Color::Rgb(42, 108, 138),
    hint: Color::Rgb(118, 92, 72),
    popup_bg: Color::Rgb(222, 232, 235),
    node_bg: Color::Rgb(220, 230, 233),
    notification_bg: Color::Rgb(221, 231, 234),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(18, 96, 128),
    swatch: Color::Rgb(10, 72, 96),
};

// ---- Infinite Rose (`INFINITE_ROSE`) ----
// Light: pale cool-gray page, dusty rose/mauve body and chrome.
pub static INFINITE_ROSE: Palette = Palette {
    name: "Infinite Rose",
    appearance: Appearance::Light,
    syntect: "infinite-rose",
    background: Color::Rgb(232, 235, 240),
    text: Color::Rgb(112, 68, 76),
    focused_border: Color::Rgb(196, 128, 118),
    tab_active_fg: Color::Rgb(255, 250, 248),
    tab_active_bg: Color::Rgb(88, 46, 54),
    tab_inactive_bg: Color::Rgb(226, 229, 234),
    search_text: Color::Rgb(148, 98, 102),
    hint: Color::Rgb(118, 108, 122),
    popup_bg: Color::Rgb(226, 229, 235),
    node_bg: Color::Rgb(224, 227, 233),
    notification_bg: Color::Rgb(225, 228, 234),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(178, 108, 98),
    swatch: Color::Rgb(112, 68, 76),
};

// ---- Barley Bound (light: Gruvbox-light riff — buttercream pad, dark warm ink, teal focus, orange brand) ----
pub static BARLEY_BOUND: Palette = Palette {
    name: "Barley Bound",
    appearance: Appearance::Light,
    syntect: "barley-bound",
    background: Color::Rgb(251, 241, 199),
    text: Color::Rgb(60, 56, 54),
    focused_border: Color::Rgb(69, 133, 136),
    tab_active_fg: Color::Rgb(251, 241, 199),
    tab_active_bg: Color::Rgb(80, 73, 69),
    tab_inactive_bg: Color::Rgb(235, 219, 178),
    search_text: Color::Rgb(121, 116, 14),
    hint: Color::Rgb(124, 111, 100),
    popup_bg: Color::Rgb(235, 219, 178),
    node_bg: Color::Rgb(232, 213, 172),
    notification_bg: Color::Rgb(234, 216, 175),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(214, 93, 14),
    swatch: Color::Rgb(60, 56, 54),
};

// ---- Verglas Trace (light: Nord-light riff — snow page, polar-night ink, frost blue chrome) ----
pub static VERGLAS_TRACE: Palette = Palette {
    name: "Verglas Trace",
    appearance: Appearance::Light,
    syntect: "verglas-trace",
    background: Color::Rgb(236, 239, 244),
    text: Color::Rgb(46, 52, 64),
    focused_border: Color::Rgb(94, 129, 172),
    tab_active_fg: Color::Rgb(236, 239, 244),
    tab_active_bg: Color::Rgb(59, 66, 82),
    tab_inactive_bg: Color::Rgb(229, 233, 240),
    search_text: Color::Rgb(129, 161, 193),
    hint: Color::Rgb(76, 86, 106),
    popup_bg: Color::Rgb(229, 233, 240),
    node_bg: Color::Rgb(226, 231, 238),
    notification_bg: Color::Rgb(228, 232, 239),
    delta_added: DEFAULT_COLORS.green,
    delta_mod: DEFAULT_COLORS.yellow,
    delta_removed: DEFAULT_COLORS.red,
    title_brand: Color::Rgb(136, 192, 208),
    swatch: Color::Rgb(46, 52, 64),
};

/// All built-in palettes. Order is irrelevant: [`theme_ordered_list`] sorts by name within dark / light.
static ALL_THEMES: &[&Palette] = &[
    &ARCHIVAL_SIMULACRA,
    &ASTERION_CODE,
    &BABEL_BLEND,
    &BARLEY_BOUND,
    &BURNING_GLYPH,
    &CRYPTIC_CHAI,
    &FADED_ECHO,
    &FROZEN_PHRASE,
    &GARDEN_UNSEEN,
    &GOLDEN_DELIRIUM,
    &OBDURATE_NOON,
    &OBLIVION_INK,
    &OCHRE_THREAD,
    &PALE_MIRROR,
    &PARCHED_PAGE,
    &PURPLE_HAZE,
    &RESIN_RECORD,
    &INFINITE_ROSE,
    &SHADOW_INDEX,
    &SILENT_SHEET,
    &TANGERINE_MEMORY,
    &VERGLAS_TRACE,
];

struct ThemeLists {
    /// Theme selector list: `"Dark"` / `"Light"` section rows, then theme rows (alphabetical under each).
    selector: Vec<SelectorEntry>,
    /// Flat list of themes only: all dark palettes A–Z, then all light A–Z (picker order, config lookup).
    ordered: Vec<&'static Palette>,
}

static THEME_LISTS: LazyLock<ThemeLists> = LazyLock::new(|| {
    let mut dark: Vec<&'static Palette> = ALL_THEMES
        .iter()
        .copied()
        .filter(|t| t.appearance == Appearance::Dark)
        .collect();
    let mut light: Vec<&'static Palette> = ALL_THEMES
        .iter()
        .copied()
        .filter(|t| t.appearance == Appearance::Light)
        .collect();
    dark.sort_by_key(|t| t.name);
    light.sort_by_key(|t| t.name);

    let mut selector = Vec::new();
    selector.push(SelectorEntry::Section("Dark"));
    for &t in &dark {
        selector.push(SelectorEntry::Item(t));
    }
    selector.push(SelectorEntry::Section("Light"));
    for &t in &light {
        selector.push(SelectorEntry::Item(t));
    }

    let ordered: Vec<&'static Palette> = selector
        .iter()
        .filter_map(|e| match e {
            SelectorEntry::Item(t) => Some(*t),
            SelectorEntry::Section(_) => None,
        })
        .collect();

    ThemeLists { selector, ordered }
});

/// All themes in picker order (dark A–Z, then light A–Z). Same order as [`theme_selector_entries`] minus section rows.
#[must_use]
pub fn theme_ordered_list() -> &'static [&'static Palette] {
    &THEME_LISTS.ordered
}

/// Rows for the in-app theme picker (sections + themes).
#[must_use]
pub fn theme_selector_entries() -> &'static [SelectorEntry] {
    &THEME_LISTS.selector
}
