//! Scoped invalidation of catalog LocalResources after Space/bulk mutations.

use std::ops::BitOr;

use leptos::prelude::*;

/// Which shared catalog payloads a mutation invalidated.
///
/// Mutations bump only what they touched, so e.g. adding rows to a lens no
/// longer refetches all of `/entries`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CatalogScope(u8);

impl CatalogScope {
    /// Nothing changed — toast only.
    pub(crate) const NONE: Self = Self(0);
    /// `/entries` and `/categories`.
    pub(crate) const ENTRIES: Self = Self(1);
    /// `/lenses` names plus per-lens members.
    pub(crate) const LENSES: Self = Self(1 << 1);
    /// `/duplicates`.
    pub(crate) const DUPLICATES: Self = Self(1 << 2);
    /// `/delta`.
    pub(crate) const DELTA: Self = Self(1 << 3);
    /// Every payload — snapshots, root switches, and path mutations.
    pub(crate) const ALL: Self =
        Self(Self::ENTRIES.0 | Self::LENSES.0 | Self::DUPLICATES.0 | Self::DELTA.0);

    const fn overlaps(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for CatalogScope {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Bump a scope so the matching [`crate::catalog_data::CatalogData`] resources refetch.
#[derive(Clone, Copy)]
pub(crate) struct CatalogRefresh {
    entries: RwSignal<u32>,
    lenses: RwSignal<u32>,
    duplicates: RwSignal<u32>,
    delta: RwSignal<u32>,
}

impl CatalogRefresh {
    pub(crate) fn provide() -> Self {
        let ctx = Self {
            entries: RwSignal::new(0),
            lenses: RwSignal::new(0),
            duplicates: RwSignal::new(0),
            delta: RwSignal::new(0),
        };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn bump(self, scope: CatalogScope) {
        for (bit, generation) in self.generations() {
            if scope.overlaps(bit) {
                generation.update(|n| *n = n.wrapping_add(1));
            }
        }
    }

    /// Tracked read of `scope`'s generation — resources subscribe to just their slice.
    ///
    /// Pass a single scope when the value is used as a cache key (see
    /// [`crate::catalog_data::CatalogData::lens_members_for`]); across combined
    /// scopes the sum only guarantees a change, not a distinct value.
    pub(crate) fn tick(self, scope: CatalogScope) -> u32 {
        self.generations()
            .into_iter()
            .filter(|(bit, _)| scope.overlaps(*bit))
            .fold(0, |acc, (_, generation)| acc.wrapping_add(generation.get()))
    }

    /// The one place scopes map to storage.
    fn generations(self) -> [(CatalogScope, RwSignal<u32>); 4] {
        [
            (CatalogScope::ENTRIES, self.entries),
            (CatalogScope::LENSES, self.lenses),
            (CatalogScope::DUPLICATES, self.duplicates),
            (CatalogScope::DELTA, self.delta),
        ]
    }
}
