//! Unicode symbols and tree-drawing characters for layout/render.

use crate::utils::StringObjTraits;

/// Unicode symbols used in layout/render. Nerd Fonts or similar may be needed for powerline characters.
pub struct UiGlyphs {
    /// Powerline-style segment: round left (curve on right). Used for tab nodes and status nodes.
    pub round_left: char,
    /// Powerline-style segment: round right (curve on left). Used for tab nodes and status nodes.
    pub round_right: char,
    /// Full block (e.g. theme selector swatch). U+2588.
    pub swatch_block: char,
    /// Short box-drawing run for theme-picker section headers (`───`); placed on both sides of **Dark** / **Light**.
    pub theme_section_rule: &'static str,
    /// Markdown viewer: suffix (after link text) for inline links `[text](url)`.
    pub markdown_link: char,
    /// Markdown viewer: suffix for link destinations that look like file attachments (.pdf, .zip, …).
    pub markdown_attachment: char,
    /// Markdown viewer: prefix for `![alt](url)` image syntax (photo / figure).
    pub markdown_image: char,
    /// Sort direction glyph for ascending/up.
    pub arrow_up: char,
    /// Sort direction glyph for descending/down.
    pub arrow_down: char,
    /// Font Awesome GitHub (`U+F09B`, Nerd Fonts PUA). Requires a Nerd-patched font to render as the logo.
    pub github_mark: char,
    /// Settings left pane: prefix when this row is focused (`›` + space).
    pub settings_row_active: &'static str,
    // Setting note marker (asterisk)
    pub settings_note_asterisk: &'static str,
    // Setting note marker (square) — typed column tables row + footnote
    pub settings_note_square: &'static str,
    // Setting note marker (arrow) — OSC 11 opacity_format row + footnote
    pub settings_note_arrow: &'static str,
    /// Two-space indent: inactive Settings row prefix and wrapped path continuation lines.
    pub indent_two_spaces: &'static str,
}

impl Default for UiGlyphs {
    fn default() -> Self {
        Self::new()
    }
}

impl StringObjTraits for UiGlyphs {
    fn new() -> Self {
        UiGlyphs::new()
    }
}

impl UiGlyphs {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            round_left: '\u{e0b6}',
            round_right: '\u{e0b4}',
            swatch_block: '\u{2588}',
            theme_section_rule: "\u{2500}\u{2500}\u{2500}",
            markdown_link: '\u{2197}',        // ↗
            markdown_attachment: '\u{1f4ce}', // 📎
            markdown_image: '\u{1f5bc}',      // 🖼 (framed picture)
            arrow_up: '\u{2191}',             // ↑
            arrow_down: '\u{2193}',           // ↓
            github_mark: '\u{f09b}',
            settings_row_active: "\u{203a} ",  // ›
            settings_note_asterisk: "* ",      // asterisk
            settings_note_square: "\u{25aa} ", // ▪ small square
            settings_note_arrow: "\u{2023} ",  // ‣ triangular bullet
            indent_two_spaces: "  ",
        }
    }
}

pub const UI_GLYPHS: UiGlyphs = UiGlyphs::new();

/// Tree-drawing characters for directory-style trees (e.g. schema tree, file tree). Single place to tweak box-drawing.
pub struct TreeChars {
    /// Non-last sibling: "├─ "
    pub branch: &'static str,
    /// Last sibling: "└─ "
    pub last_branch: &'static str,
    /// Continuation (more siblings below): "│  "
    pub vertical: &'static str,
    /// No continuation (last branch): "   "
    pub space: &'static str,
}

impl Default for TreeChars {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeChars {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            branch: "├─ ",
            last_branch: "└─ ",
            vertical: "│  ",
            space: "   ",
        }
    }
}

pub const TREE_CHARS: TreeChars = TreeChars::new();
