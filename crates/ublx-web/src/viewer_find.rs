//! In-pane Viewer find (TUI Shift+S) — right `title_bottom` strip + DOM highlights.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node, Text};

use crate::panes::RightTab;

/// Label matches TUI `UI_STRINGS.search.find_label`.
pub(crate) const FIND_LABEL: &str = "Search: ";

const HIT_CLASS: &str = "viewer-find-hit";
const HIT_CURRENT: &str = "viewer-find-hit--current";
const ROOT_SEL: &str = ".right-pane .panel-pad";

/// Shift+S find strip state + DOM highlight ticks.
#[derive(Clone, Copy)]
pub(crate) struct ViewerFind {
    pub query: ReadSignal<String>,
    pub set_query: WriteSignal<String>,
    pub active: ReadSignal<bool>,
    pub set_active: WriteSignal<bool>,
    pub committed: ReadSignal<bool>,
    pub set_committed: WriteSignal<bool>,
    pub current: ReadSignal<usize>,
    pub set_current: WriteSignal<usize>,
    pub total: ReadSignal<usize>,
    pub set_total: WriteSignal<usize>,
    /// Bumped when right-pane body HTML/text remounts so marks re-sync.
    pub content_tick: ReadSignal<u32>,
    pub set_content_tick: WriteSignal<u32>,
    /// Typing, committed, or non-empty query (TUI `title_bottom_visible`).
    pub strip_visible: Signal<bool>,
    /// Drive highlights (TUI `find_affects_view`).
    pub find_affects_view: Signal<bool>,
}

impl ViewerFind {
    pub(crate) fn provide() -> Self {
        let (query, set_query) = signal(String::new());
        let (active, set_active) = signal(false);
        let (committed, set_committed) = signal(false);
        let (current, set_current) = signal(0usize);
        let (total, set_total) = signal(0usize);
        let (content_tick, set_content_tick) = signal(0u32);
        let strip_visible = Signal::derive(move || {
            active.get() || committed.get() || !query.get().trim().is_empty()
        });
        let find_affects_view = Signal::derive(move || {
            (active.get() || committed.get()) && !query.get().trim().is_empty()
        });
        let ctx = Self {
            query,
            set_query,
            active,
            set_active,
            committed,
            set_committed,
            current,
            set_current,
            total,
            set_total,
            content_tick,
            set_content_tick,
            strip_visible,
            find_affects_view,
        };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn bump_content(self) {
        self.set_content_tick.update(|n| *n = n.wrapping_add(1));
    }

    pub(crate) fn start(self) {
        self.set_active.set(true);
        self.set_committed.set(false);
    }

    pub(crate) fn submit(self) {
        self.set_active.set(false);
        self.set_committed.set(true);
    }

    pub(crate) fn clear(self) {
        self.set_query.set(String::new());
        self.set_active.set(false);
        self.set_committed.set(false);
        self.set_current.set(0);
        self.set_total.set(0);
        clear_marks_in_right_pane();
    }

    pub(crate) fn next(self) {
        let n = self.total.get_untracked();
        if n == 0 {
            return;
        }
        let cur = self.current.get_untracked();
        self.set_current.set((cur + 1) % n);
    }

    pub(crate) fn prev(self) {
        let n = self.total.get_untracked();
        if n == 0 {
            return;
        }
        let cur = self.current.get_untracked();
        self.set_current
            .set(cur.checked_sub(1).unwrap_or(n.saturating_sub(1)));
    }
}

/// Re-apply marks whenever find state / right tab / content tick changes.
pub(crate) fn install_highlight_effect(find: ViewerFind, tab: ReadSignal<RightTab>) {
    Effect::new(move |_| {
        let affects = find.find_affects_view.get();
        let needle = find.query.get();
        let current = find.current.get();
        let case_insensitive = tab.get() == RightTab::Metadata;
        let _ = tab.get();
        let _ = find.content_tick.get();

        if !affects {
            clear_marks_in_right_pane();
            find.set_total.set(0);
            return;
        }

        // Wait a frame so HostHtmlBody / KvTables can paint.
        let Some(window) = web_sys::window() else {
            return;
        };
        let find = find;
        let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
            let total = apply_marks(&needle, case_insensitive, current);
            find.set_total.set(total);
            if total > 0 && current >= total {
                find.set_current.set(total.saturating_sub(1));
            }
            scroll_current_into_view();
        }) as Box<dyn FnMut()>);
        let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
        cb.forget();
    });
}

fn clear_marks_in_right_pane() {
    let Some(root) = right_pane_pad() else {
        return;
    };
    clear_marks(&root);
}

fn right_pane_pad() -> Option<Element> {
    web_sys::window()?
        .document()?
        .query_selector(ROOT_SEL)
        .ok()
        .flatten()
}

fn clear_marks(root: &Element) {
    let Ok(marks) = root.query_selector_all(&format!("mark.{HIT_CLASS}")) else {
        return;
    };
    // Walk backwards so live NodeList stays stable enough.
    for i in (0..marks.length()).rev() {
        let Some(node) = marks.item(i) else {
            continue;
        };
        let Ok(mark) = node.dyn_into::<Element>() else {
            continue;
        };
        let Some(parent) = mark.parent_node() else {
            continue;
        };
        while let Some(child) = mark.first_child() {
            let _ = parent.insert_before(&child, Some(mark.as_ref()));
        }
        let _ = parent.remove_child(&mark);
        if let Ok(el) = parent.dyn_into::<Element>() {
            el.normalize();
        }
    }
}

fn apply_marks(needle: &str, case_insensitive: bool, current: usize) -> usize {
    let Some(root) = right_pane_pad() else {
        return 0;
    };
    clear_marks(&root);
    let needle = needle.trim();
    if needle.is_empty() {
        return 0;
    }

    let texts = collect_text_nodes(root.as_ref());
    if texts.is_empty() {
        return 0;
    }

    let mut haystack = String::new();
    let mut spans: Vec<(Text, usize)> = Vec::with_capacity(texts.len());
    for t in &texts {
        spans.push((t.clone(), haystack.len()));
        haystack.push_str(&t.data());
    }

    let ranges = if case_insensitive {
        literal_match_ranges_ascii_insensitive(&haystack, needle)
    } else {
        literal_match_ranges(&haystack, needle)
    };
    let total = ranges.len();
    if total == 0 {
        return 0;
    }

    // Wrap from the end so earlier offsets stay valid.
    for (idx, &(start, end)) in ranges.iter().enumerate().rev() {
        let is_current = idx == current.min(total.saturating_sub(1));
        wrap_haystack_range(&spans, start, end, is_current);
    }
    total
}

fn literal_match_ranges(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    haystack
        .match_indices(needle)
        .map(|(i, m)| (i, i + m.len()))
        .collect()
}

fn literal_match_ranges_ascii_insensitive(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    let hb = haystack.as_bytes();
    let nb = needle.as_bytes();
    if nb.is_empty() || hb.len() < nb.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + nb.len() <= hb.len() {
        if hb[i..i + nb.len()].eq_ignore_ascii_case(nb) {
            out.push((i, i + nb.len()));
            i += nb.len();
        } else {
            i += 1;
        }
    }
    out
}

fn collect_text_nodes(root: &Node) -> Vec<Text> {
    let mut out = Vec::new();
    collect_text_nodes_into(root, &mut out);
    out
}

fn collect_text_nodes_into(node: &Node, out: &mut Vec<Text>) {
    if node.node_type() == Node::TEXT_NODE {
        if let Ok(t) = node.clone().dyn_into::<Text>() {
            // Skip empty / whitespace-only-only? Keep all — matches TUI haystack.
            out.push(t);
        }
        return;
    }
    if let Some(el) = node.dyn_ref::<Element>() {
        let tag = el.tag_name().to_ascii_lowercase();
        if matches!(tag.as_str(), "script" | "style" | "noscript") {
            return;
        }
        // Don't descend into our own marks when re-scanning mid-clear.
        if tag == "mark" && el.class_list().contains(HIT_CLASS) {
            return;
        }
    }
    let children = node.child_nodes();
    for i in 0..children.length() {
        if let Some(child) = children.item(i) {
            collect_text_nodes_into(&child, out);
        }
    }
}

fn wrap_haystack_range(spans: &[(Text, usize)], start: usize, end: usize, is_current: bool) {
    if start >= end {
        return;
    }
    // Process each text node overlap from the end of the list.
    for (text, node_start) in spans.iter().rev() {
        let len = text.length() as usize;
        let node_end = node_start + len;
        if node_end <= start || *node_start >= end {
            continue;
        }
        let local_start = start.saturating_sub(*node_start).min(len);
        let local_end = end.saturating_sub(*node_start).min(len);
        if local_start >= local_end {
            continue;
        }
        wrap_text_slice(text, local_start, local_end, is_current);
    }
}

fn wrap_text_slice(text: &Text, start: usize, end: usize, is_current: bool) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(mark) = doc.create_element("mark") else {
        return;
    };
    let _ = mark.class_list().add_1(HIT_CLASS);
    if is_current {
        let _ = mark.class_list().add_1(HIT_CURRENT);
    }

    // splitText: offset is UTF-16 code units (DOM). Our haystack is UTF-8 `String` from
    // `text.data()` — for BMP-only catalog text this matches; non-BMP is rare in paths/code.
    let start_u32 = start as u32;
    let end_u32 = end as u32;
    let len = text.length();
    if end_u32 > len || start_u32 > end_u32 {
        return;
    }

    // Split into: [0, start) | [start, end) | [end, len)
    let mid = if start_u32 == 0 {
        text.clone()
    } else {
        match text.split_text(start_u32) {
            Ok(t) => t,
            Err(_) => return,
        }
    };
    let match_len = end_u32 - start_u32;
    if match_len < mid.length() {
        let _ = mid.split_text(match_len);
    }

    let Some(parent) = mid.parent_node() else {
        return;
    };
    if parent
        .insert_before(mark.as_ref(), Some(mid.as_ref()))
        .is_err()
    {
        return;
    }
    let _ = mark.append_child(mid.as_ref());
}

fn scroll_current_into_view() {
    let Some(root) = right_pane_pad() else {
        return;
    };
    let Ok(cur) = root.query_selector(&format!("mark.{HIT_CLASS}.{HIT_CURRENT}")) else {
        return;
    };
    let Some(el) = cur else {
        return;
    };
    let Ok(html) = el.dyn_into::<HtmlElement>() else {
        return;
    };
    let opts = web_sys::ScrollIntoViewOptions::new();
    opts.set_block(web_sys::ScrollLogicalPosition::Center);
    opts.set_inline(web_sys::ScrollLogicalPosition::Nearest);
    html.scroll_into_view_with_scroll_into_view_options(&opts);
}

/// Find strip on right pane `title_bottom` (left of size / page meta).
#[component]
pub(crate) fn ViewerFindStrip() -> impl IntoView {
    let find = ViewerFind::expect();
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let has_find_count = Signal::derive(move || find.total.get() > 0);

    Effect::new(move |_| {
        if find.active.get()
            && let Some(el) = input_ref.get()
        {
            let _ = el.focus();
        }
    });

    view! {
        <div
            class=move || {
                if find.active.get() {
                    "viewer-find viewer-find--active"
                } else if find.committed.get() {
                    "viewer-find viewer-find--committed"
                } else {
                    "viewer-find"
                }
            }
            on:click=move |_| find.start()
        >
            <span class="viewer-find__label">{FIND_LABEL}</span>
            <Show
                when=move || find.active.get()
                fallback=move || {
                    view! {
                        <span class="viewer-find__query">{move || find.query.get()}</span>
                    }
                }
            >
                <input
                    node_ref=input_ref
                    class="viewer-find__input"
                    type="text"
                    prop:value=move || find.query.get()
                    aria-label="Find in right pane"
                    on:input=move |ev| {
                        find.set_query.set(event_target_value(&ev));
                        find.set_current.set(0);
                    }
                    on:keydown=move |ev| {
                        ev.stop_propagation();
                        match ev.key().as_str() {
                            "Enter" => {
                                ev.prevent_default();
                                find.submit();
                            }
                            "Escape" => {
                                ev.prevent_default();
                                find.clear();
                            }
                            _ => {}
                        }
                    }
                    on:blur=move |_| {
                        // Keep strip visible with query; leave typing mode like catalog search.
                        if find.active.get_untracked() {
                            find.set_active.set(false);
                            if !find.query.get_untracked().trim().is_empty() {
                                find.set_committed.set(true);
                            }
                        }
                    }
                />
            </Show>
            <Show when=move || has_find_count.get()>
                <span class="viewer-find__count">
                    {move || {
                        let t = find.total.get();
                        let c = find.current.get().min(t.saturating_sub(1)) + 1;
                        format!("{c}/{t}")
                    }}
                </span>
            </Show>
        </div>
    }
}
