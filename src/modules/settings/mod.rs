//! **Settings** main-mode UI: bool rows, layout editor, TOML overlay sync, and tab entry.

mod bool_rows;
mod command_mode_leader_row;
mod context;
mod layout_edit;
mod sync;
mod tab;
mod typed_column_tables_row;

pub use bool_rows::*;
pub use command_mode_leader_row::*;
pub use context::*;
pub use layout_edit::*;
pub use sync::*;
pub use tab::*;
pub use typed_column_tables_row::*;

#[cfg(test)]
mod tests;
