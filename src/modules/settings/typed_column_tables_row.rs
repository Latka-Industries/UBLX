//! Settings row for [`crate::config::ColumnStatsDisplay`] on `typed_column_tables` (`none` | `abbrev` | `full`).

use crate::config::{ColumnStatsDisplay, UblxOverlay};
use crate::layout::setup::SettingsConfigScope;
use crate::ui::UI_STRINGS;

use super::bool_rows::bool_row_count;

/// Row index for `typed_column_tables`, directly after bool rows (Global and Local).
#[must_use]
pub fn typed_column_tables_row_index(scope: SettingsConfigScope) -> usize {
    bool_row_count(scope)
}

pub const TYPED_COLUMN_TABLES_VARIANTS: [ColumnStatsDisplay; 3] = [
    ColumnStatsDisplay::None,
    ColumnStatsDisplay::Abbrev,
    ColumnStatsDisplay::Full,
];

#[must_use]
pub fn overlay_typed_column_tables(overlay: &UblxOverlay) -> ColumnStatsDisplay {
    overlay.typed_column_tables.unwrap_or_default()
}

#[must_use]
pub fn local_typed_column_tables_is_explicit(local: Option<&UblxOverlay>) -> bool {
    local.is_some_and(|l| l.typed_column_tables.is_some())
}

pub fn write_typed_column_tables(overlay: &mut UblxOverlay, value: ColumnStatsDisplay) {
    overlay.typed_column_tables = Some(value);
}

#[must_use]
pub fn typed_column_tables_button_label(v: ColumnStatsDisplay) -> &'static str {
    let s = &UI_STRINGS.settings_pane;
    match v {
        ColumnStatsDisplay::None => s.typed_column_tables_none,
        ColumnStatsDisplay::Abbrev => s.typed_column_tables_abbrev,
        ColumnStatsDisplay::Full => s.typed_column_tables_full,
    }
}

#[must_use]
pub fn typed_column_tables_toml_value(v: ColumnStatsDisplay) -> &'static str {
    match v {
        ColumnStatsDisplay::None => "none",
        ColumnStatsDisplay::Abbrev => "abbrev",
        ColumnStatsDisplay::Full => "full",
    }
}

#[must_use]
pub fn cycle_typed_column_tables(current: ColumnStatsDisplay) -> ColumnStatsDisplay {
    match current {
        ColumnStatsDisplay::None => ColumnStatsDisplay::Abbrev,
        ColumnStatsDisplay::Abbrev => ColumnStatsDisplay::Full,
        ColumnStatsDisplay::Full => ColumnStatsDisplay::None,
    }
}
