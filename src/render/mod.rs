//! Frame drawing: main layout ([`core`]), panes and overlays, file/markdown/CSV viewers, key/value
//! metadata tables, scrollable areas, and related widgets.

pub mod core;
pub mod kv_tables;
pub mod marquee;
pub mod overlays;
pub mod panes;
pub mod path_lines;
pub mod scrollable_content;
pub mod viewer_cache;
pub mod viewers;

pub use core::*;
