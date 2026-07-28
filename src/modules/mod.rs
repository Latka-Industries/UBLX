//! Handler logic for small, named features (search, ublx-settings, theme-selector, dupe-finder, lens).

pub mod enhancer;
#[cfg(feature = "tui")]
pub mod exporter;
pub mod file_ops;
#[cfg(feature = "tui")]
mod finders;
#[cfg(feature = "tui")]
pub mod first_run;
pub mod lenses;
#[cfg(feature = "tui")]
pub mod opener;
pub mod settings;
#[cfg(feature = "tui")]
pub mod theme_selector;
#[cfg(feature = "tui")]
pub mod ublx_switch;

#[cfg(feature = "tui")]
pub use finders::*;
