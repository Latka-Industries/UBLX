//! Indexing engine: `SQLite` [`db_ops`], in-memory [`cache`], walk/orchestration ([`orchestrator`]), and
//! off-thread viewer work ([`viewer_async`]).

#[cfg(feature = "tui")]
pub mod cache;
pub mod db_ops;
pub mod orchestrator;
#[cfg(feature = "tui")]
pub mod viewer_async;

#[cfg(test)]
mod tests;
