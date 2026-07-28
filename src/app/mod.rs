//! TUI app loop and view building. The loop runs in [`main_loop`]; setup/teardown live in [`crate::handlers::core::run_tui_session`].

#[cfg(feature = "tui")]
mod delta;
#[cfg(feature = "tui")]
mod params;
#[cfg(feature = "tui")]
mod runtime;
#[cfg(feature = "tui")]
mod snapshot;
pub mod tokio_rt;
#[cfg(feature = "tui")]
mod user_selected;
#[cfg(feature = "tui")]
mod view_data;

#[cfg(feature = "tui")]
pub use params::*;
#[cfg(feature = "tui")]
pub use runtime::main_loop;
#[cfg(feature = "tui")]
pub use snapshot::load_snapshot_for_tui;
