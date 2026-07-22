//! Middle-pane content sort — mirrors TUI [`ContentSort`](../../../../src/layout/setup.rs)
//! / [`sort_node_text`](../../../../src/render/panes/middle.rs).

use leptos::prelude::*;

use crate::nav::MainMode;

/// ↑ / ↓ glyphs match TUI `UI_GLYPHS.arrow_*`.
const ARROW_UP: char = '\u{2191}';
const ARROW_DOWN: char = '\u{2193}';

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    #[must_use]
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    #[must_use]
    fn arrow(self) -> char {
        // TUI: Asc → ↓, Desc → ↑
        match self {
            Self::Asc => ARROW_DOWN,
            Self::Desc => ARROW_UP,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SnapshotSortKey {
    #[default]
    Name,
    Size,
    Mod,
}

impl SnapshotSortKey {
    #[must_use]
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Mod,
            Self::Mod => Self::Name,
        }
    }

    #[must_use]
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Size => "Size",
            Self::Mod => "Mod",
        }
    }
}

/// Mode-aware content sort (same fields / cycle rules as TUI `ContentSort`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ContentSort {
    pub snapshot_key: SnapshotSortKey,
    pub snapshot_dir: SortDirection,
    pub delta_dir: SortDirection,
}

impl ContentSort {
    #[must_use]
    pub(crate) fn cycle_for_mode(self, main_mode: MainMode) -> Self {
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

    /// TUI `sort_node_text` — `None` for Lenses / Settings (counter only).
    #[must_use]
    pub(crate) fn node_text(self, main_mode: MainMode) -> Option<String> {
        match main_mode {
            MainMode::Snapshot | MainMode::Duplicates => Some(format!(
                "{} {}",
                self.snapshot_key.label(),
                self.snapshot_dir.arrow()
            )),
            MainMode::Delta => Some(format!("Time {}", self.delta_dir.arrow())),
            MainMode::Lenses | MainMode::Settings => None,
        }
    }
}

/// Shared sort state for the shell (one `ContentSort` like TUI `panels.content_sort`).
#[derive(Clone, Copy)]
pub(crate) struct ContentSortCtx {
    pub sort: ReadSignal<ContentSort>,
    pub set_sort: WriteSignal<ContentSort>,
}

impl ContentSortCtx {
    pub(crate) fn provide() -> Self {
        let (sort, set_sort) = signal(ContentSort::default());
        let ctx = Self { sort, set_sort };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn cycle(self, main_mode: MainMode) {
        if matches!(main_mode, MainMode::Lenses | MainMode::Settings) {
            return;
        }
        self.set_sort.update(|s| *s = s.cycle_for_mode(main_mode));
    }
}

/// Sort snapshot-style rows by Name / Size / Mod (+ direction). Path is `(path, size, mtime_ns)`.
pub(crate) fn sort_snapshot_rows(rows: &mut [(String, u64, Option<i64>)], sort: ContentSort) {
    match sort.snapshot_key {
        SnapshotSortKey::Name => {
            rows.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        }
        SnapshotSortKey::Size => {
            rows.sort_unstable_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        }
        SnapshotSortKey::Mod => {
            rows.sort_unstable_by(|a, b| {
                let am = a.2.unwrap_or(i64::MIN);
                let bm = b.2.unwrap_or(i64::MIN);
                am.cmp(&bm).then_with(|| a.0.cmp(&b.0))
            });
        }
    }
    if sort.snapshot_dir == SortDirection::Desc {
        rows.reverse();
    }
}

/// Sort delta rows by `created_ns` then path (TUI `sort_delta_rows_by_time`).
pub(crate) fn sort_delta_rows(rows: &mut [(i64, String)], sort: ContentSort) {
    rows.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    if sort.delta_dir == SortDirection::Desc {
        rows.reverse();
    }
}
