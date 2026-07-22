//! Format file content by type for the viewer (markdown, csv, image, json, etc.).

pub mod async_tools;
pub mod csv_handler;
pub mod html_escape;
pub mod images;
pub mod markdown;
pub mod pdf_preview;
pub mod pretty_tables;
pub mod svg_preview;
pub mod syntect_text;
pub mod video_preview;

pub use html_escape::html_escape_minimal;
