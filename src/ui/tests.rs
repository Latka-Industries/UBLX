//! Main tab bar mode/label ordering.

use crate::engine::db_ops::DuplicateGroupingMode;
use crate::layout::setup::MainMode;
use crate::ui::main_tab_bar_modes_and_labels;

#[test]
fn main_tab_bar_modes_order_no_optional() {
    let (modes, labels) = main_tab_bar_modes_and_labels(false, false, DuplicateGroupingMode::Hash);
    assert_eq!(
        modes,
        vec![MainMode::Snapshot, MainMode::Delta, MainMode::Settings,]
    );
    assert_eq!(modes.len(), labels.len());
}

#[test]
fn main_tab_bar_modes_order_all_optional() {
    let (modes, labels) = main_tab_bar_modes_and_labels(true, true, DuplicateGroupingMode::Hash);
    assert_eq!(
        modes,
        vec![
            MainMode::Snapshot,
            MainMode::Lenses,
            MainMode::Delta,
            MainMode::Duplicates,
            MainMode::Settings,
        ]
    );
    assert_eq!(modes.len(), labels.len());
}

#[test]
fn main_tab_bar_modes_lenses_only() {
    let (modes, _) = main_tab_bar_modes_and_labels(true, false, DuplicateGroupingMode::Hash);
    assert_eq!(
        modes,
        vec![
            MainMode::Snapshot,
            MainMode::Lenses,
            MainMode::Delta,
            MainMode::Settings,
        ]
    );
}

#[test]
fn main_tab_bar_modes_duplicates_only() {
    let (modes, _) = main_tab_bar_modes_and_labels(false, true, DuplicateGroupingMode::Hash);
    assert_eq!(
        modes,
        vec![
            MainMode::Snapshot,
            MainMode::Delta,
            MainMode::Duplicates,
            MainMode::Settings,
        ]
    );
}
