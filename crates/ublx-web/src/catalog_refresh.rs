//! Bump to refetch catalog LocalResources after Space/bulk mutations.

use leptos::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct CatalogRefresh {
    pub tick: RwSignal<u32>,
}

impl CatalogRefresh {
    pub(crate) fn provide() -> Self {
        let ctx = Self {
            tick: RwSignal::new(0),
        };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn bump(self) {
        self.tick.update(|n| *n = n.wrapping_add(1));
    }
}
