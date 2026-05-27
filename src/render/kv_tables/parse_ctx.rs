//! Shared parse context for metadata JSON → table sections (inline array width + column-stats mode).

use crate::config::ColumnStatsDisplay;

use super::format;

/// Width and column-stats settings passed through JSON walk / section parse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KvParseCtx {
    pub max_array_inline: usize,
    pub column_stats: ColumnStatsDisplay,
}

impl KvParseCtx {
    #[must_use]
    pub fn new(max_array_inline: usize, column_stats: ColumnStatsDisplay) -> Self {
        Self {
            max_array_inline: max_array_inline.max(1),
            column_stats,
        }
    }

    /// Match [`super::draw::draw_tables`] array inline cap from a padded table width.
    #[must_use]
    pub fn from_table_width(table_width: u16, column_stats: ColumnStatsDisplay) -> Self {
        let value_w = format::value_width_from_table_width(table_width);
        Self::new(
            format::max_array_inline_for_value_width(value_w),
            column_stats,
        )
    }

    /// Match metadata-tab find / haystack formatting from content text width.
    #[must_use]
    pub fn from_content_width(content_width: u16, column_stats: ColumnStatsDisplay) -> Self {
        Self::new(
            format::max_array_inline_for_value_width(content_width),
            column_stats,
        )
    }
}
