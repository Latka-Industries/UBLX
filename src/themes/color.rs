//! Palette color type. Under `tui`, this is ratatui's [`ratatui::style::Color`].
//! Without `tui` (serve / query-only), a minimal local RGB/Reset enum so themes
//! compile without linking ratatui (THI-213).

#[cfg(feature = "tui")]
pub use ratatui::style::Color;

/// Minimal color set for non-TUI builds (palettes + CSS tokens only use RGB / Reset).
#[cfg(not(feature = "tui"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    Rgb(u8, u8, u8),
    Reset,
}
