//! Middle-pane multi-select (TUI `ui/multiselect.rs` subset).
//!
//! **Ctrl+Space** enter/exit (Snapshot / Lenses + contents focus only).
//! **Space** toggles the cursor row while active. **Esc** exits.
//! Bulk / Space menus → mini-PR 14.

use std::collections::HashSet;

use leptos::prelude::*;

use crate::nav::MainMode;

/// Block glyph — TUI [`UI_GLYPHS.swatch_block`].
pub(crate) const CHECK_GLYPH: &str = "\u{2588}";

#[derive(Clone, Copy)]
pub(crate) struct MultiselectCtx {
    pub active: RwSignal<bool>,
    pub selected: RwSignal<HashSet<String>>,
    /// Middle-pane cursor path (Snapshot / Lenses); PathsPane keeps this fresh.
    pub cursor: RwSignal<Option<String>>,
}

impl MultiselectCtx {
    pub(crate) fn provide() -> Self {
        let ctx = Self {
            active: RwSignal::new(false),
            selected: RwSignal::new(HashSet::new()),
            cursor: RwSignal::new(None),
        };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn clear(self) {
        self.active.set(false);
        self.selected.set(HashSet::new());
    }

    pub(crate) fn applies(mode: MainMode) -> bool {
        matches!(mode, MainMode::Snapshot | MainMode::Lenses)
    }

    /// Ctrl+Space — returns whether the key was consumed.
    pub(crate) fn try_toggle_mode(self, mode: MainMode, middle_focused: bool) -> bool {
        if !Self::applies(mode) || !middle_focused {
            return false;
        }
        let next = !self.active.get_untracked();
        if next {
            self.active.set(true);
            if let Some(p) = self.cursor.get_untracked() {
                self.selected.update(|s| {
                    s.insert(p);
                });
            }
        } else {
            self.clear();
        }
        true
    }

    /// Space while active — toggle cursor path. Always consumes when active.
    pub(crate) fn toggle_row(self) {
        let Some(p) = self.cursor.get_untracked() else {
            return;
        };
        self.selected.update(|s| {
            if !s.remove(&p) {
                s.insert(p);
            }
        });
    }

    pub(crate) fn is_checked(self, path: &str) -> bool {
        self.active.get() && self.selected.with(|s| s.contains(path))
    }

    pub(crate) fn count(self) -> usize {
        if self.active.get() {
            self.selected.with(HashSet::len)
        } else {
            0
        }
    }
}
