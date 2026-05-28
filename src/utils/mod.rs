//! Shared utilities: path/clipboard/terminal helpers, byte formatting, notifications, and small I/O
//! affordances (not the TUI or engine proper).

mod clipboard;
mod embedded_cover;
mod error_writer;
mod external_tools;
mod format;
mod notifications;
mod path;
mod perf;
mod terminal_osc;
mod tet_info;
mod tools;

pub use clipboard::*;
pub use embedded_cover::*;
pub use error_writer::*;
pub use external_tools::*;
pub use format::*;
pub use notifications::*;
pub use path::*;
pub use perf::*;
pub use terminal_osc::*;
pub use tet_info::*;
pub use tools::*;
