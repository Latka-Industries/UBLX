//! Shared UI layout constants (padding, constraints).

#[cfg(feature = "tui")]
use ratatui::layout::Constraint;
#[cfg(feature = "tui")]
use ratatui::style::Style;
#[cfg(feature = "tui")]
use ratatui::text::Span;

/// Shared UI layout constants (padding, etc.). Constraint arrays are derived from the scalar values via the `*_constraints()` methods.
pub struct UiConstants {
    pub h_pad: u16,
    pub v_pad: u16,
    pub popup_padding_w: u16,
    pub popup_padding_h: u16,
    /// Theme-picker swatch for **dark** themes on a **dark** popup: HSL lighten off page background via [`crate::themes::adjust_surface_rgb`].
    pub swatch_lighten: f32,
    /// Same as [`Self::swatch_lighten`] but when the picker is shown while a **light** theme is active — stronger lighten so chips are not mud-on-cream.
    pub swatch_lighten_dark_on_light_popup: f32,
    /// Theme-picker swatch for **light** themes: [`crate::themes::lighten_rgb`] on body text (try 0.2–0.4).
    pub swatch_light_theme_text: f32,
    pub table_stripe_lighten: f32,
    pub input_poll_ms: u64,
    pub status_line_height: u16,
    pub tab_row_height: u16,
    /// Blank line between main tab bar and 3-pane body.
    pub tab_body_gap_height: u16,
    /// Horizontal gap (terminal cells) between main tab nodes; must match mouse hit-testing.
    pub main_tab_node_gap_cells: u16,
    pub brand_block_width: u16,
    pub empty_space: &'static str,
}

impl Default for UiConstants {
    fn default() -> Self {
        Self::new()
    }
}

impl UiConstants {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            h_pad: 1,
            v_pad: 1,
            popup_padding_w: 4,
            popup_padding_h: 2,
            swatch_lighten: 0.2,
            swatch_lighten_dark_on_light_popup: 0.38,
            swatch_light_theme_text: 0.6,
            table_stripe_lighten: 0.06,
            input_poll_ms: 100,
            status_line_height: 1,
            tab_row_height: 1,
            tab_body_gap_height: 1,
            main_tab_node_gap_cells: 1,
            brand_block_width: 4,
            empty_space: " ",
        }
    }

    /// Main area (min 1 row) + status line. Derived from [`Self::status_line_height`].
    #[cfg(feature = "tui")]
    #[must_use]
    pub fn status_line_constraints(&self) -> [Constraint; 2] {
        [
            Constraint::Min(1),
            Constraint::Length(self.status_line_height),
        ]
    }

    /// Tab row + gap + body. Derived from [`Self::tab_row_height`] and [`Self::tab_body_gap_height`].
    #[cfg(feature = "tui")]
    #[must_use]
    pub fn tab_row_constraints(&self) -> [Constraint; 3] {
        [
            Constraint::Length(self.tab_row_height),
            Constraint::Length(self.tab_body_gap_height),
            Constraint::Min(1),
        ]
    }

    /// Tabs (flex) + brand block. Derived from [`Self::brand_block_width`].
    #[cfg(feature = "tui")]
    #[must_use]
    pub fn brand_block_constraints(&self) -> [Constraint; 2] {
        [
            Constraint::Min(0),
            Constraint::Length(self.brand_block_width),
        ]
    }

    #[cfg(feature = "tui")]
    #[must_use]
    pub fn get_empty_span(&self, style: Style) -> Span<'static> {
        Span::styled(self.empty_space, style)
    }
}

pub const UI_CONSTANTS: UiConstants = UiConstants::new();
