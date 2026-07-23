//! Key/value table rendering for right-pane **Metadata** and **Writing** JSON (from zahirscan).
//! Layout, padding, and scrollbar live in [`crate::render::scrollable_content`]; this crate parses
//! JSON into sections and draws only the visible window (ratatui’s `Table` has no native scroll).
//!
//! ## Stable API (rest of the app)
//!
//! Prefer the crate-root re-exports below. Call sites today: [`draw_right_pane`](crate::render::panes::draw_right_pane) / fullscreen.
//!
//! - [`draw_tables`] — per-frame draw with scroll offset.
//! - [`content_height`] — total line count for scrollbar math (must match draw).
//! - [`parse_json_sections`] — only if you need `Section` values outside draw (tests, tooling).
//!
//! Submodules (`walk`, `column_metadata`, `schema`, …) stay `pub` for a shallow module tree and tests; treat
//! them as implementation details unless you are extending parsing.
//!
//! ## Hot path vs cache
//!
//! **Per frame, no parsed-section cache:** [`content_height`] and [`draw_tables`] each call
//! [`parse_json_sections`], so large JSON can be parsed twice in one paint when the right pane
//! shows metadata/writing. (Scroll width iteration may call [`content_height`] multiple times in
//! the same frame.) Row **virtualization** applies only inside [`draw_tables`]: Contents tables
//! build ratatui rows for the visible slice, not for the full entry list.
//!
//! Multi-blob JSON (`"\n\n"`-separated objects) uses Rayon in [`parse_json_sections`] when blob
//! count ≥ [`PARALLEL.json_sections_blobs`](crate::config::PARALLEL).
//!
//! Longer module map: see `README.md` in this directory.

pub mod column_metadata;
pub mod consts;
pub mod draw;
pub mod export;
pub mod format;
pub mod parse_ctx;
pub mod ratatui_table;
pub mod schema;
pub mod sections;
pub mod tabular;
pub mod walk;
pub mod xlsx;

pub use draw::*;
pub use export::{SectionView, TreeNodeView, parse_json_to_views, tree_node_to_view};
pub use parse_ctx::KvParseCtx;
pub use schema::tree_roots_to_lines;
pub use sections::*;
pub use walk::WalkKeyVars;

#[cfg(test)]
mod tests;
