//! Shell-owned catalog resources shared across mode mounts.

use std::collections::HashMap;

use leptos::prelude::*;

use crate::api::{
    DeltaCatalog, DuplicatesResponse, EntryRow, fetch_delta_catalog, fetch_duplicates,
    fetch_lens_entries, fetch_lens_names, get_json,
};
use crate::catalog_refresh::{CatalogRefresh, CatalogScope};

/// Catalog payloads shared by main modes.
///
/// Living in shell context means tab remounts reuse the same `LocalResource`s
/// instead of re-hitting `/entries`, `/categories`, `/duplicates`, `/delta`,
/// and `/lenses`. Each resource tracks only its own [`CatalogScope`], so a
/// refresh refetches just the payloads the mutation invalidated.
///
/// Lens **members** are memoized per lens name (see [`Self::lens_members_for`])
/// and dropped when the lens tick advances.
#[derive(Clone, Copy)]
pub(crate) struct CatalogData {
    pub entries: LocalResource<Vec<EntryRow>>,
    pub categories: LocalResource<Vec<String>>,
    pub duplicates: LocalResource<DuplicatesResponse>,
    pub delta: LocalResource<DeltaCatalog>,
    pub lens_names: LocalResource<Vec<String>>,
    /// Per-lens path lists; generation tracked in [`Self::lens_members_tick`].
    lens_members: RwSignal<HashMap<String, Vec<EntryRow>>>,
    lens_members_tick: RwSignal<u32>,
}

impl CatalogData {
    pub(crate) fn provide(refresh: CatalogRefresh) -> Self {
        let categories = LocalResource::new(move || {
            let _ = refresh.tick(CatalogScope::ENTRIES);
            async move {
                get_json::<Vec<String>>("/categories")
                    .await
                    .unwrap_or_default()
            }
        });
        let entries = LocalResource::new(move || {
            let _ = refresh.tick(CatalogScope::ENTRIES);
            async move {
                get_json::<Vec<EntryRow>>("/entries")
                    .await
                    .unwrap_or_default()
            }
        });
        let duplicates = LocalResource::new(move || {
            let _ = refresh.tick(CatalogScope::DUPLICATES);
            async move { fetch_duplicates().await }
        });
        let delta = LocalResource::new(move || {
            let _ = refresh.tick(CatalogScope::DELTA);
            async move { fetch_delta_catalog().await }
        });
        let lens_names = LocalResource::new(move || {
            let _ = refresh.tick(CatalogScope::LENSES);
            async move { fetch_lens_names().await }
        });

        let data = Self {
            entries,
            categories,
            duplicates,
            delta,
            lens_names,
            lens_members: RwSignal::new(HashMap::new()),
            lens_members_tick: RwSignal::new(0),
        };
        provide_context(data);
        data
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    /// Members for `name` at lens `tick`, using the shell cache (fetch on miss).
    pub(crate) async fn lens_members_for(self, name: Option<String>, tick: u32) -> Vec<EntryRow> {
        let Some(n) = name else {
            return Vec::new();
        };
        if self.lens_members_tick.get_untracked() != tick {
            self.lens_members.set(HashMap::new());
            self.lens_members_tick.set(tick);
        }
        if let Some(hit) = self.lens_members.with_untracked(|m| m.get(&n).cloned()) {
            return hit;
        }
        let rows = fetch_lens_entries(&n).await;
        self.lens_members.update(|m| {
            m.insert(n, rows.clone());
        });
        rows
    }
}
