//! Shared pane focus + list navigation slots for keyboard control.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

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

/// Pane focus + left/middle list-nav callback slots.
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

/// Right-pane tab request / cycle (digits / Tab from keys).
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

/// Active PDF Viewer page callbacks (registered while mounted).
#[derive(Clone)]
pub(crate) struct PdfPageCtl {
    pub apply: Callback<PdfPageNav>,
    /// Jump to an absolute page (caller clamps to `1..=page_count`).
    pub goto: Callback<u32>,
    pub page: Signal<u32>,
    pub page_count: Signal<Option<u32>>,
}

/// Explore #12: byte-windowed plain text (Shift+J/K/B/E advance windows when mounted).
#[derive(Clone)]
pub(crate) struct TextWindowCtl {
    pub apply: Callback<PdfPageNav>,
    pub offset: Signal<u64>,
    pub byte_len: Signal<u64>,
    pub total: Signal<u64>,
}

/// Expand / collapse every `.schema-tree` `<details>` in a mounted tree.
#[derive(Clone)]
pub(crate) struct TreeCollapseCtl {
    pub expand_all: Callback<()>,
    pub collapse_all: Callback<()>,
    /// True when at least one collapsible node is closed.
    pub can_expand: Signal<bool>,
    /// True when at least one collapsible node is open.
    pub can_collapse: Signal<bool>,
}

/// Preview key targets — PDF pages, text windows, schema trees.
#[derive(Clone, Copy)]
pub(crate) struct PreviewKeysBus {
    pub pdf: RwSignal<Option<PdfPageCtl>>,
    pub text_win: RwSignal<Option<TextWindowCtl>>,
    pub tree: RwSignal<Option<TreeCollapseCtl>>,
}

impl PreviewKeysBus {
    pub(crate) fn provide() -> Self {
        let pdf = RwSignal::new(None::<PdfPageCtl>);
        let text_win = RwSignal::new(None::<TextWindowCtl>);
        let tree = RwSignal::new(None::<TreeCollapseCtl>);
        let bus = Self {
            pdf,
            text_win,
            tree,
        };
        provide_context(bus);
        bus
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }
}

/// Wire expand/collapse + enabled state to `root` (must contain `.schema-tree__node` details).
pub(crate) fn install_tree_collapse_on(root: &web_sys::HtmlElement) {
    let preview = PreviewKeysBus::expect();
    let (can_expand, set_can_expand) = signal(false);
    let (can_collapse, set_can_collapse) = signal(false);
    let root = root.clone();

    let ctl = TreeCollapseCtl {
        expand_all: Callback::new({
            let root = root.clone();
            move |_| {
                set_tree_details_open(&root, true);
                sync_tree_open_state(&root, set_can_expand, set_can_collapse);
            }
        }),
        collapse_all: Callback::new({
            let root = root.clone();
            move |_| {
                set_tree_details_open(&root, false);
                sync_tree_open_state(&root, set_can_expand, set_can_collapse);
            }
        }),
        can_expand: can_expand.into(),
        can_collapse: can_collapse.into(),
    };
    preview.tree.set(Some(ctl));

    // Children may paint after the host mounts — attach toggle listeners next frame.
    let root_wire = root.clone();
    let raf = wasm_bindgen::closure::Closure::once(move || {
        if let Ok(nodes) = root_wire.query_selector_all("details.schema-tree__node") {
            for i in 0..nodes.length() {
                let Some(node) = nodes.item(i) else {
                    continue;
                };
                let Ok(details) = node.dyn_into::<web_sys::HtmlDetailsElement>() else {
                    continue;
                };
                let root_toggle = root_wire.clone();
                let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |_: web_sys::Event| {
                    sync_tree_open_state(&root_toggle, set_can_expand, set_can_collapse);
                })
                    as Box<dyn FnMut(_)>);
                let _ =
                    details.add_event_listener_with_callback("toggle", cb.as_ref().unchecked_ref());
                cb.forget();
            }
        }
        sync_tree_open_state(&root_wire, set_can_expand, set_can_collapse);
    });
    if let Some(win) = web_sys::window() {
        let _ = win.request_animation_frame(raf.as_ref().unchecked_ref());
    }
    std::mem::forget(raf);

    on_cleanup(move || {
        preview.tree.set(None);
    });
}

fn set_tree_details_open(root: &web_sys::HtmlElement, open: bool) {
    let Ok(nodes) = root.query_selector_all("details.schema-tree__node") else {
        return;
    };
    for i in 0..nodes.length() {
        let Some(node) = nodes.item(i) else {
            continue;
        };
        if let Ok(details) = node.dyn_into::<web_sys::HtmlDetailsElement>() {
            details.set_open(open);
        }
    }
}

fn sync_tree_open_state(
    root: &web_sys::HtmlElement,
    set_can_expand: WriteSignal<bool>,
    set_can_collapse: WriteSignal<bool>,
) {
    let Ok(nodes) = root.query_selector_all("details.schema-tree__node") else {
        set_can_expand.set(false);
        set_can_collapse.set(false);
        return;
    };
    let mut any_open = false;
    let mut any_closed = false;
    for i in 0..nodes.length() {
        let Some(node) = nodes.item(i) else {
            continue;
        };
        let Ok(details) = node.dyn_into::<web_sys::HtmlDetailsElement>() else {
            continue;
        };
        if details.open() {
            any_open = true;
        } else {
            any_closed = true;
        }
    }
    set_can_expand.set(any_closed);
    set_can_collapse.set(any_open);
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
