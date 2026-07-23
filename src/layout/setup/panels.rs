//! Left/middle list panels and content sort.

use std::time::Instant;

use ratatui::style::Style;
use ratatui::widgets::ListState;

use super::super::style;
use super::modes::{MainMode, PanelFocus};

/// State for horizontal marquee when a list row label overflows (e.g. Duplicates/Lenses left pane).
#[derive(Debug, Default)]
pub struct ContentMarqueeState {
    pub offset: usize,
    pub last_advance: Option<Instant>,
    pub anchor: Option<(usize, String)>,
}

impl ContentMarqueeState {
    pub fn reset(&mut self) {
        self.offset = 0;
        self.last_advance = None;
        self.anchor = None;
    }
}

/// List panels: categories, contents, focus, preview scroll, and highlight style.
#[derive(Default)]
pub struct PanelState {
    pub category_state: ListState,
    pub content_state: ListState,
    pub focus: PanelFocus,
    pub preview_scroll: u16,
    pub prev_preview_key: Option<(usize, Option<usize>)>,
    pub highlight_style: Style,
    pub content_sort: ContentSort,
    /// Temporary anchor used to keep the same selected item identity after sort changes.
    pub sort_anchor_path: Option<String>,
    /// Last converged right-pane body text width (for find footer + tab match counts).
    pub right_pane_text_w: Option<u16>,
    /// Marquee for the left category list in Duplicates / Lenses when the selected name overflows.
    pub category_marquee: ContentMarqueeState,
    /// Marquee for the middle contents path list when the selected row overflows (Snapshot / Delta / Duplicates / Lenses).
    pub content_marquee: ContentMarqueeState,
    /// Zahir compact `columns` table verbosity in Metadata / Writing tabs.
    pub typed_column_tables: crate::config::ColumnStatsDisplay,
}

impl PanelState {
    pub(super) fn new() -> Self {
        let mut p = Self {
            highlight_style: style::list_highlight(),
            ..Default::default()
        };
        p.category_state.select(Some(0));
        p.content_state.select(Some(0));
        p
    }
}

/// Middle-pane sort direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

/// Snapshot/Duplicates middle-pane sort key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnapshotSortKey {
    #[default]
    Name,
    Size,
    Mod,
}

impl SnapshotSortKey {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Mod,
            Self::Mod => Self::Name,
        }
    }
}

/// Mode-aware content sort state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentSort {
    pub snapshot_key: SnapshotSortKey,
    pub snapshot_dir: SortDirection,
    pub delta_dir: SortDirection,
}

impl ContentSort {
    #[must_use]
    pub fn cycle_for_mode(self, main_mode: MainMode) -> Self {
        match main_mode {
            MainMode::Snapshot | MainMode::Duplicates => {
                if self.snapshot_dir == SortDirection::Asc {
                    Self {
                        snapshot_dir: SortDirection::Desc,
                        ..self
                    }
                } else {
                    Self {
                        snapshot_key: self.snapshot_key.next(),
                        snapshot_dir: SortDirection::Asc,
                        ..self
                    }
                }
            }
            MainMode::Delta => Self {
                delta_dir: self.delta_dir.next(),
                ..self
            },
            MainMode::Lenses | MainMode::Settings => self,
        }
    }
}
