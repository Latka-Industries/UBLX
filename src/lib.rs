pub mod app;
pub mod cli;
pub mod cli_parser;
pub mod config;
pub mod engine;
pub mod handlers;
pub mod integrations;
pub mod layout;
pub mod modules;
pub mod render;
pub mod themes;
pub mod ui;
pub mod utils;

/// Workspace catalog crate (paths / db_ops / headless open-read). Re-exported for THI-155 Phase 2.
pub use ublx_catalog as catalog;
