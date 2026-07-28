//! Frame drawing: main layout ([`core`]), panes and overlays, file/markdown/CSV viewers, key/value
//! metadata tables, scrollable areas, and related widgets.

#[cfg(feature = "tui")]
pub mod core;
pub mod kv_tables;
#[cfg(feature = "tui")]
pub mod marquee;
#[cfg(feature = "tui")]
pub mod overlays;
#[cfg(feature = "tui")]
pub mod panes;
#[cfg(feature = "tui")]
pub mod path_lines;
#[cfg(feature = "tui")]
pub mod scrollable_content;
pub mod templates;
#[cfg(feature = "tui")]
pub mod viewer_cache;
pub mod viewers;

#[cfg(feature = "tui")]
pub use core::*;
