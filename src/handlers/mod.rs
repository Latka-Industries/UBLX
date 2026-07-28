//! TUI event handlers: session lifecycle ([`core`]), snapshot/background pipeline, mode transitions, and
//! the right-hand preview stack ([`viewing`]).

#[cfg(feature = "tui")]
mod core;
/// Serve-safe zahir/dir preview helpers (THI-213).
pub mod preview;
mod snapshot_pipeline;
#[cfg(feature = "tui")]
pub mod state_transitions;
#[cfg(feature = "tui")]
pub mod viewing;

#[cfg(feature = "tui")]
pub use core::*;
pub use preview::{directory_tree_nodes, sectioned_preview_from_zahir};
pub use snapshot_pipeline::*;
