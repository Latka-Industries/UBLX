//! `CommandModeCtx` — chord FSM + pickers (keyboard-safe: no `expect_context` in hot path).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::SettingsScope;
use crate::catalog_refresh::CatalogRefresh;
use crate::nav::MainMode;
use crate::toast::ToastCtx;

use super::actions::{
    move_picker, open_theme_selector, restore_theme_preview, run_letter, submit_picker,
};
use super::helpers::sleep_ms;
use super::rows::{CHORD_MENU_DELAY_MS, DEFAULT_LEADER, description_for};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Picker {
    Theme {
        rows: Vec<crate::api::ThemePickerRow>,
        /// Index into theme items only (skips Dark/Light section rows).
        selected: usize,
        /// CSS to restore when dismissing without committing (TUI `before_selector`).
        restore: crate::api::ThemeCssBody,
    },
    Root {
        paths: Vec<String>,
        selected: usize,
    },
}

impl Picker {
    pub(super) fn theme_count(&self) -> usize {
        match self {
            Self::Theme { rows, .. } => rows.iter().filter(|r| r.theme_name().is_some()).count(),
            Self::Root { paths, .. } => paths.len(),
        }
    }

    pub(super) fn selected_theme_name(&self) -> Option<&str> {
        let Self::Theme { rows, selected, .. } = self else {
            return None;
        };
        crate::api::ThemePickerRow::theme_name_at(rows, *selected)
    }

    pub(super) fn selected_theme_css(&self) -> Option<&crate::api::ThemeCssBody> {
        let Self::Theme { rows, selected, .. } = self else {
            return None;
        };
        crate::api::ThemePickerRow::theme_css_at(rows, *selected)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CommandModeCtx {
    pub pending: RwSignal<bool>,
    pub menu_visible: RwSignal<bool>,
    pub leader: RwSignal<char>,
    pub(super) picker: RwSignal<Option<Picker>>,
    /// Scope used when opening / committing the theme picker (Command Mode → Local).
    pub(super) theme_scope: RwSignal<SettingsScope>,
    /// Bumped after a successful theme commit (Settings can refetch).
    pub theme_committed: RwSignal<u32>,
    /// Generation so stale chord timers do not open the menu after Esc.
    chord_gen: RwSignal<u32>,
    pub(super) refresh: CatalogRefresh,
    pub(super) set_mode: WriteSignal<MainMode>,
    toasts: ToastCtx,
}

impl CommandModeCtx {
    pub(crate) fn provide(
        refresh: CatalogRefresh,
        set_mode: WriteSignal<MainMode>,
        toasts: ToastCtx,
    ) -> Self {
        let ctx = Self {
            pending: RwSignal::new(false),
            menu_visible: RwSignal::new(false),
            leader: RwSignal::new(DEFAULT_LEADER),
            picker: RwSignal::new(None),
            theme_scope: RwSignal::new(SettingsScope::Local),
            theme_committed: RwSignal::new(0),
            chord_gen: RwSignal::new(0),
            refresh,
            set_mode,
            toasts,
        };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn is_active(self) -> bool {
        self.pending.get_untracked()
            || self.menu_visible.get_untracked()
            || self.picker.get_untracked().is_some()
    }

    pub(crate) fn end_chord(self) {
        self.pending.set(false);
        self.menu_visible.set(false);
        self.chord_gen.update(|n| *n = n.wrapping_add(1));
    }

    pub(crate) fn close_all(self) {
        restore_theme_preview(self);
        self.end_chord();
        self.picker.set(None);
    }

    pub(crate) fn flash(self, msg: impl Into<String>) {
        self.toasts.info(msg);
    }

    pub(crate) fn flash_warn(self, msg: impl Into<String>) {
        self.toasts.warn(msg);
    }

    pub(crate) fn flash_err(self, msg: impl Into<String>) {
        self.toasts.error(msg);
    }

    pub(crate) fn flash_snapshot(self, added: usize, modified: usize, removed: usize) {
        self.toasts.snapshot_done(added, modified, removed);
    }

    /// Start Ctrl+leader wait; menu appears after [`CHORD_MENU_DELAY_MS`] if still pending.
    pub(crate) fn begin_chord(self) {
        restore_theme_preview(self);
        self.picker.set(None);
        self.pending.set(true);
        self.menu_visible.set(false);
        self.chord_gen.update(|n| *n = n.wrapping_add(1));
        let generation = self.chord_gen.get_untracked();
        spawn_local(async move {
            sleep_ms(CHORD_MENU_DELAY_MS).await;
            if self.chord_gen.get_untracked() != generation {
                return;
            }
            if self.pending.get_untracked() && !self.menu_visible.get_untracked() {
                self.menu_visible.set(true);
            }
        });
    }

    /// Letter while chord active (fast path or menu). Returns true if handled.
    pub(crate) fn submit_hotkey(self, c: char) -> bool {
        if self.picker.get_untracked().is_some() {
            return false;
        }
        let c = c.to_ascii_lowercase();
        if description_for(c).is_none() {
            self.end_chord();
            return true;
        }
        self.end_chord();
        run_letter(self, c);
        true
    }

    pub(crate) fn picker_open(self) -> bool {
        self.picker.get_untracked().is_some()
    }

    pub(crate) fn picker_move(self, delta: i32) {
        move_picker(self, delta);
    }

    pub(crate) fn picker_submit(self) {
        submit_picker(self);
    }

    /// Open the theme selector overlay (Command Mode `t` / Settings theme control).
    pub(crate) fn open_theme_selector(self, scope: SettingsScope) {
        self.end_chord();
        open_theme_selector(self, scope);
    }
}
