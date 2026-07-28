//! **Settings** overlay helpers. TUI tab/layout editors are behind `tui` (THI-213).

mod bool_rows;
#[cfg(feature = "tui")]
mod command_mode_leader_row;
#[cfg(feature = "tui")]
mod context;
#[cfg(feature = "tui")]
mod layout_edit;
#[cfg(feature = "tui")]
mod sync;
#[cfg(feature = "tui")]
mod tab;
mod typed_column_tables_row;

pub use bool_rows::*;
#[cfg(feature = "tui")]
pub use command_mode_leader_row::*;
#[cfg(feature = "tui")]
pub use context::*;
#[cfg(feature = "tui")]
pub use layout_edit::*;
#[cfg(feature = "tui")]
pub use sync::*;
#[cfg(feature = "tui")]
pub use tab::*;
pub use typed_column_tables_row::*;

#[cfg(all(test, feature = "tui"))]
mod tests;
