//! `SQLite` **`.ublx`** database ops. Moved to the `ublx-catalog` workspace crate (THI-155 Phase 2);
//! this module re-exports it so `crate::engine::db_ops::…` call sites keep working.

pub use ublx_catalog::db_ops::*;
