//! Shared pane focus + list navigation slots for keyboard control.

use leptos::prelude::*;

use crate::panes::RightTab;

/// Keyboard focus is left or middle only — right pane is content/tabs, not a focus target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PaneFocus {
    Left,
    #[default]
    Middle,
}

impl PaneFocus {
    pub(crate) fn cycle(self) -> Self {
        match self {
            Self::Left => Self::Middle,
            Self::Middle => Self::Left,
        }
    }
}

/// Callbacks for the focused list (left or middle).
#[derive(Clone)]
pub(crate) struct ListNav {
    pub move_by: Callback<i32>,
    pub to_start: Callback<()>,
    pub to_end: Callback<()>,
}

#[derive(Clone, Copy)]
pub(crate) struct UiNav {
    pub pane: ReadSignal<PaneFocus>,
    pub set_pane: WriteSignal<PaneFocus>,
    pub left: RwSignal<Option<ListNav>>,
    pub middle: RwSignal<Option<ListNav>>,
}

impl UiNav {
    pub(crate) fn provide() -> (Self, RightTabBus, PreviewKeysBus) {
        let (pane, set_pane) = signal(PaneFocus::Middle);
        let left = RwSignal::new(None::<ListNav>);
        let middle = RwSignal::new(None::<ListNav>);
        let (right_req, set_right_req) = signal(None::<RightTab>);
        let (cycle_tick, bump_cycle) = signal(0u32);

        let nav = Self {
            pane,
            set_pane,
            left,
            middle,
        };
        let tabs = RightTabBus {
            request: right_req,
            set_request: set_right_req,
            cycle_tick,
            bump_cycle,
        };
        let preview = PreviewKeysBus::provide();
        provide_context(nav);
        provide_context(tabs);
        (nav, tabs, preview)
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn active_list(self) -> Option<ListNav> {
        match self.pane.get_untracked() {
            PaneFocus::Left => self.left.get_untracked(),
            PaneFocus::Middle => self.middle.get_untracked(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RightTabBus {
    pub request: ReadSignal<Option<RightTab>>,
    pub set_request: WriteSignal<Option<RightTab>>,
    pub cycle_tick: ReadSignal<u32>,
    pub bump_cycle: WriteSignal<u32>,
}

impl RightTabBus {
    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }
}

/// PDF page control while a PDF Viewer is mounted (Shift+J/K/B/E → pages, else DOM scroll).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PdfPageNav {
    Next,
    Prev,
    Top,
    Bottom,
}

#[derive(Clone)]
pub(crate) struct PdfPageCtl {
    pub apply: Callback<PdfPageNav>,
    /// Jump to an absolute page (caller clamps to `1..=page_count`).
    pub goto: Callback<u32>,
    pub page: Signal<u32>,
    pub page_count: Signal<Option<u32>>,
}

#[derive(Clone, Copy)]
pub(crate) struct PreviewKeysBus {
    pub pdf: RwSignal<Option<PdfPageCtl>>,
}

impl PreviewKeysBus {
    pub(crate) fn provide() -> Self {
        let pdf = RwSignal::new(None::<PdfPageCtl>);
        let bus = Self { pdf };
        provide_context(bus);
        bus
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }
}

/// Build list nav for a `selected: Option<String>` over ordered keys.
pub(crate) fn string_list_nav(
    keys: Signal<Vec<String>>,
    selected: Signal<Option<String>>,
    set_selected: WriteSignal<Option<String>>,
) -> ListNav {
    let step = move |delta: i32| {
        let ks = keys.get_untracked();
        if ks.is_empty() {
            return;
        }
        let cur = selected.get_untracked();
        let idx = cur.as_ref().and_then(|s| ks.iter().position(|k| k == s));
        let next = match (idx, delta.cmp(&0)) {
            (None, std::cmp::Ordering::Less) => ks.len() - 1,
            (None, _) => 0,
            (Some(i), _) => {
                let n = ks.len() as i32;
                ((i as i32 + delta).clamp(0, n - 1)) as usize
            }
        };
        set_selected.set(ks.get(next).cloned());
    };

    ListNav {
        move_by: Callback::new(move |d: i32| step(d)),
        to_start: Callback::new(move |_| {
            let ks = keys.get_untracked();
            set_selected.set(ks.first().cloned());
        }),
        to_end: Callback::new(move |_| {
            let ks = keys.get_untracked();
            set_selected.set(ks.last().cloned());
        }),
    }
}

/// List nav over ordered `usize` ids (Duplicates left pane).
pub(crate) fn id_list_nav(
    ids: Signal<Vec<usize>>,
    selected: Signal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
) -> ListNav {
    let step = move |delta: i32| {
        let ks = ids.get_untracked();
        if ks.is_empty() {
            return;
        }
        let cur = selected.get_untracked();
        let idx = cur.and_then(|id| ks.iter().position(|k| *k == id));
        let next = match (idx, delta.cmp(&0)) {
            (None, std::cmp::Ordering::Less) => ks.len() - 1,
            (None, _) => 0,
            (Some(i), _) => {
                let n = ks.len() as i32;
                ((i as i32 + delta).clamp(0, n - 1)) as usize
            }
        };
        set_selected.set(ks.get(next).copied());
    };

    ListNav {
        move_by: Callback::new(move |d: i32| step(d)),
        to_start: Callback::new(move |_| {
            let ks = ids.get_untracked();
            set_selected.set(ks.first().copied());
        }),
        to_end: Callback::new(move |_| {
            let ks = ids.get_untracked();
            set_selected.set(ks.last().copied());
        }),
    }
}

/// Register `nav` into a slot while this reactive scope lives.
pub(crate) fn install_list_nav(slot: RwSignal<Option<ListNav>>, nav: ListNav) {
    slot.set(Some(nav));
    on_cleanup(move || {
        slot.set(None);
    });
}
