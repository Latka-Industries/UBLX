//! Windowed Snapshot Contents fetch (THI-207) — range requests, not a full `/entries` dump.

use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{EntriesListQuery, EntryRow, fetch_entries_page};
use crate::catalog_refresh::{CatalogRefresh, CatalogScope};

/// Below this, one-shot `limit=total` (keeps client Name/Size/Mod sort).
pub(crate) const ENTRIES_FAST_PATH_MAX: usize = 50_000;
/// Fetch chunk size for scroll / selection overscan.
pub(crate) const ENTRIES_PAGE_SIZE: usize = 256;
/// Placeholder key prefix for unloaded windowed rows (`\0pending:{index}`).
pub(crate) const PENDING_KEY_PREFIX: &str = "\0pending:";
const SEARCH_DEBOUNCE_MS: i32 = 200;

/// Path + category + sort fields (no zahir).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlimEntry {
    pub path: String,
    pub category: String,
    pub size: u64,
    pub mtime_ns: Option<i64>,
}

impl From<EntryRow> for SlimEntry {
    fn from(r: EntryRow) -> Self {
        Self {
            path: r.path,
            category: r.category,
            size: r.size,
            mtime_ns: r.mtime_ns,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct FilterKey {
    category: Option<String>,
    contains: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadMode {
    /// Full filtered set in [`EntriesWindow::dense`].
    Dense,
    /// Sparse [`EntriesWindow::cache`] keyed by absolute index.
    Windowed,
}

/// Snapshot Contents list backed by `GET /entries?limit=&offset=` (+ filters).
#[derive(Clone, Copy)]
pub(crate) struct EntriesWindow {
    /// Catalog refresh generation (ENTRIES scope).
    epoch: RwSignal<u32>,
    filter: RwSignal<FilterKey>,
    mode: RwSignal<LoadMode>,
    total: RwSignal<usize>,
    dense: RwSignal<Option<Arc<Vec<SlimEntry>>>>,
    cache: RwSignal<HashMap<usize, SlimEntry>>,
    /// In-flight range fetches (ignore stale gens).
    inflight: RwSignal<u32>,
    ready: RwSignal<bool>,
}

impl EntriesWindow {
    pub(crate) fn provide(
        refresh: CatalogRefresh,
        flags: RwSignal<crate::api::CatalogFlags>,
    ) -> Self {
        let store = Self {
            epoch: RwSignal::new(0),
            filter: RwSignal::new(FilterKey::default()),
            mode: RwSignal::new(LoadMode::Windowed),
            total: RwSignal::new(0),
            dense: RwSignal::new(None),
            cache: RwSignal::new(HashMap::new()),
            inflight: RwSignal::new(0),
            ready: RwSignal::new(false),
        };
        provide_context(store);

        // Only the root *string* (Memo) — not every `flags.set` chrome refresh.
        let root = Memo::new(move |_| flags.with(|f| f.root.clone()));
        Effect::new(move |_| {
            let _ = refresh.tick(CatalogScope::ENTRIES);
            let _ = root.get();
            store.invalidate_and_bootstrap(true);
        });

        store
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    /// Drop rows immediately on root switch (before awaits) so the previous list cannot linger.
    pub(crate) fn wipe_for_root(self) {
        self.invalidate_and_bootstrap(true);
    }

    pub(crate) fn ready(self) -> Signal<bool> {
        self.ready.into()
    }

    pub(crate) fn total(self) -> Signal<usize> {
        self.total.into()
    }

    pub(crate) fn is_dense(self) -> Signal<bool> {
        Signal::derive(move || self.mode.get() == LoadMode::Dense)
    }

    pub(crate) fn dense_rows(self) -> Signal<Option<Arc<Vec<SlimEntry>>>> {
        self.dense.into()
    }

    /// Current server `category=` filter (for Snapshot effects).
    pub(crate) fn filter_category(self) -> Option<String> {
        self.filter.get_untracked().category.clone()
    }

    pub(crate) fn set_category(self, category: Option<String>) {
        let prev = self.filter.get_untracked();
        if prev.category == category {
            return;
        }
        self.filter.update(|f| f.category = category);
        self.invalidate_and_bootstrap(false);
    }

    /// Apply `contains=` (caller debounces typing; clear/search-submit can call immediately).
    pub(crate) fn set_contains(self, contains: String) {
        let next = contains.trim().to_string();
        let prev = self.filter.get_untracked();
        if prev.contains == next {
            return;
        }
        self.filter.update(|f| f.contains = next);
        self.invalidate_and_bootstrap(false);
    }

    fn invalidate_and_bootstrap(self, reset_filter: bool) {
        self.epoch.update(|g| *g = g.wrapping_add(1).max(1));
        if reset_filter {
            self.filter.set(FilterKey::default());
        }
        self.clear_rows();
        self.bootstrap();
    }

    fn clear_rows(self) {
        self.ready.set(false);
        // Leave Dense immediately so Snapshot unmounts the old PathsPane (THI-207 root switch).
        self.mode.set(LoadMode::Windowed);
        self.dense.set(None);
        self.cache.set(HashMap::new());
        self.total.set(0);
    }

    /// Bumps on every catalog wipe — Snapshot keys PathsPane remounts off this.
    pub(crate) fn list_generation(self) -> Signal<u32> {
        self.epoch.into()
    }

    pub(crate) fn search_debounce_ms() -> i32 {
        SEARCH_DEBOUNCE_MS
    }

    fn query(self, offset: usize, limit: usize) -> EntriesListQuery {
        let f = self.filter.get_untracked();
        EntriesListQuery {
            category: f.category.clone(),
            contains: (!f.contains.is_empty()).then_some(f.contains.clone()),
            offset,
            limit,
        }
    }

    fn bootstrap(self) {
        let epoch = self.epoch.get_untracked();
        self.inflight.update(|n| *n += 1);
        spawn_local(async move {
            let page = match fetch_entries_page(&self.query(0, 1)).await {
                Ok(p) => p,
                Err(_) => {
                    self.inflight.update(|n| *n = n.saturating_sub(1));
                    if self.epoch.get_untracked() == epoch {
                        self.ready.set(true);
                    }
                    return;
                }
            };
            if self.epoch.get_untracked() != epoch {
                self.inflight.update(|n| *n = n.saturating_sub(1));
                return;
            }
            self.total.set(page.total);
            if page.total == 0 {
                self.mode.set(LoadMode::Dense);
                self.dense.set(Some(Arc::new(Vec::new())));
                self.ready.set(true);
                self.inflight.update(|n| *n = n.saturating_sub(1));
                return;
            }
            if page.total <= ENTRIES_FAST_PATH_MAX {
                self.mode.set(LoadMode::Dense);
                match fetch_entries_page(&self.query(0, page.total.max(1))).await {
                    Ok(full) if self.epoch.get_untracked() == epoch => {
                        let rows: Vec<SlimEntry> =
                            full.entries.into_iter().map(SlimEntry::from).collect();
                        self.total.set(full.total);
                        self.dense.set(Some(Arc::new(rows)));
                        self.ready.set(true);
                    }
                    _ => {
                        if self.epoch.get_untracked() == epoch {
                            self.ready.set(true);
                        }
                    }
                }
            } else {
                self.mode.set(LoadMode::Windowed);
                self.apply_page(epoch, page);
                let first_end = ENTRIES_PAGE_SIZE.min(self.total.get_untracked().max(1));
                self.ensure_range(0, first_end);
                self.ready.set(true);
            }
            self.inflight.update(|n| *n = n.saturating_sub(1));
        });
    }

    fn apply_page(self, epoch: u32, page: crate::api::EntryListPage) {
        if self.epoch.get_untracked() != epoch {
            return;
        }
        self.total.set(page.total);
        self.cache.update(|m| {
            for (i, row) in page.entries.into_iter().enumerate() {
                m.insert(page.offset + i, SlimEntry::from(row));
            }
        });
    }

    /// Ensure `[start, end)` is loaded (windowed mode). No-op when dense / empty.
    pub(crate) fn ensure_range(self, start: usize, end: usize) {
        if self.mode.get_untracked() != LoadMode::Windowed {
            return;
        }
        let total = self.total.get_untracked();
        if total == 0 || start >= total {
            return;
        }
        let end = end.min(total).max(start);
        let epoch = self.epoch.get_untracked();

        let missing: Vec<usize> = self
            .cache
            .with_untracked(|m| (start..end).filter(|i| !m.contains_key(i)).collect());
        if missing.is_empty() {
            return;
        }
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        let mut r0 = missing[0];
        let mut r1 = missing[0] + 1;
        for &i in missing.iter().skip(1) {
            if i == r1 {
                r1 = i + 1;
            } else {
                ranges.push((r0, r1));
                r0 = i;
                r1 = i + 1;
            }
        }
        ranges.push((r0, r1));

        for (a, b) in ranges {
            let total = self.total.get_untracked();
            let (offset, limit) = if b >= total.saturating_sub(ENTRIES_PAGE_SIZE) {
                let limit = ENTRIES_PAGE_SIZE.min(total.max(1));
                (total.saturating_sub(limit), limit)
            } else {
                let offset = (a / ENTRIES_PAGE_SIZE) * ENTRIES_PAGE_SIZE;
                let limit = (b - offset).clamp(ENTRIES_PAGE_SIZE, ENTRIES_PAGE_SIZE * 2);
                (offset, limit.min(10_000))
            };
            self.inflight.update(|n| *n += 1);
            spawn_local(async move {
                if let Ok(page) = fetch_entries_page(&self.query(offset, limit)).await {
                    self.apply_page(epoch, page);
                }
                self.inflight.update(|n| *n = n.saturating_sub(1));
            });
        }
    }

    /// Labels for absolute `[start, end)` — `"Loading..."` when not yet fetched.
    pub(crate) fn window_rows(self, start: usize, end: usize) -> Vec<(String, String)> {
        let total = self.total.get();
        let end = end.min(total);
        let start = start.min(end);
        if self.mode.get() == LoadMode::Dense {
            let Some(rows) = self.dense.get() else {
                return Vec::new();
            };
            return rows[start..end]
                .iter()
                .map(|r| (r.path.clone(), r.path.clone()))
                .collect();
        }
        self.cache.with(|m| {
            (start..end)
                .map(|i| match m.get(&i) {
                    Some(r) => (r.path.clone(), r.path.clone()),
                    None => (
                        String::from("Loading..."),
                        format!("{PENDING_KEY_PREFIX}{i}"),
                    ),
                })
                .collect()
        })
    }

    /// Bumps when sparse cache gains rows (drives window_rows Memo).
    pub(crate) fn cache_revision(self) -> Signal<u32> {
        Signal::derive(move || {
            let n = self.cache.with(HashMap::len) as u32;
            let t = self.total.get() as u32;
            n.wrapping_add(t.wrapping_mul(1_000_003))
        })
    }

    pub(crate) fn path_at(self, index: usize) -> Option<String> {
        self.entry_at(index).map(|e| e.path.clone())
    }

    pub(crate) fn index_of_path(self, path: &str) -> Option<usize> {
        if self.mode.get_untracked() == LoadMode::Dense {
            return self
                .dense
                .get_untracked()
                .and_then(|v| v.iter().position(|r| r.path == path));
        }
        self.cache
            .with_untracked(|m| m.iter().find(|(_, r)| r.path == path).map(|(i, _)| *i))
    }

    pub(crate) fn category_of_path(self, path: &str) -> Option<String> {
        if self.mode.get_untracked() == LoadMode::Dense {
            return self.dense.get_untracked().and_then(|v| {
                v.iter()
                    .find(|r| r.path == path)
                    .map(|r| r.category.clone())
            });
        }
        self.cache.with_untracked(|m| {
            m.values()
                .find(|r| r.path == path)
                .map(|r| r.category.clone())
        })
    }

    fn entry_at(self, index: usize) -> Option<SlimEntry> {
        if self.mode.get_untracked() == LoadMode::Dense {
            return self
                .dense
                .get_untracked()
                .and_then(|v| v.get(index).cloned());
        }
        self.cache.with_untracked(|m| m.get(&index).cloned())
    }

    /// Dense-mode path→category map (empty when windowed).
    pub(crate) fn dense_path_categories(self) -> HashMap<String, String> {
        let Some(rows) = self.dense.get() else {
            return HashMap::new();
        };
        rows.iter()
            .map(|r| (r.path.clone(), r.category.clone()))
            .collect()
    }
}
