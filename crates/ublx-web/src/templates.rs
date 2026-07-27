//! Structured Templates accordion (THI-177 web) — multi-expand, filterable.

use std::collections::{BTreeMap, HashSet};

use leptos::prelude::*;
use serde::Deserialize;

/// Host `TemplateView` JSON from `GET /entries/…?zahir=1` (`template_views`).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct TemplateView {
    pub pattern: String,
    pub count: usize,
    #[serde(default)]
    pub examples: BTreeMap<String, Vec<String>>,
}

impl TemplateView {
    fn matches_filter(&self, q: &str) -> bool {
        if q.is_empty() {
            return true;
        }
        if self.pattern.to_lowercase().contains(q) {
            return true;
        }
        self.examples.iter().any(|(ph, vals)| {
            ph.to_lowercase().contains(q) || vals.iter().any(|x| x.to_lowercase().contains(q))
        })
    }
}

/// Expandable pattern list with filter + expand/collapse all.
#[component]
pub(crate) fn TemplatesPane(views: Vec<TemplateView>) -> impl IntoView {
    let filter = RwSignal::new(String::new());
    // First pattern open by default when present.
    let open = RwSignal::new({
        let mut s = HashSet::new();
        if !views.is_empty() {
            s.insert(0usize);
        }
        s
    });
    let n = views.len();

    let can_expand_all = move || open.get().len() < n;
    let can_collapse_all = move || !open.get().is_empty();

    let filtered = move || {
        let q = filter.get().trim().to_lowercase();
        views
            .iter()
            .enumerate()
            .filter(|(_, v)| v.matches_filter(&q))
            .map(|(i, v)| (i, v.clone()))
            .collect::<Vec<_>>()
    };

    view! {
        <div class="templates-pane">
            <div class="templates-pane__toolbar">
                <input
                    class="templates-pane__filter"
                    type="search"
                    placeholder="Filter patterns…"
                    prop:value=move || filter.get()
                    on:input=move |ev| filter.set(event_target_value(&ev))
                />
                <div class="templates-pane__actions">
                    <button
                        type="button"
                        class="templates-pane__btn"
                        prop:disabled=move || !can_expand_all()
                        on:click=move |_| {
                            open.update(|s| {
                                s.clear();
                                s.extend(0..n);
                            });
                        }
                    >
                        "Expand all"
                    </button>
                    <button
                        type="button"
                        class="templates-pane__btn"
                        prop:disabled=move || !can_collapse_all()
                        on:click=move |_| open.update(|s| s.clear())
                    >
                        "Collapse all"
                    </button>
                </div>
            </div>
            <div class="templates-pane__list" role="list">
                <For
                    each=filtered
                    key=|(i, _)| *i
                    children=move |(i, view)| {
                        let pattern = view.pattern.clone();
                        let count = view.count;
                        let examples = view.examples.clone();
                        let is_open = move || open.get().contains(&i);
                        view! {
                            <details
                                class="templates-pane__item"
                                prop:open=is_open
                                on:toggle=move |ev| {
                                    let el = event_target::<web_sys::HtmlDetailsElement>(&ev);
                                    open.update(|s| {
                                        if el.open() {
                                            s.insert(i);
                                        } else {
                                            s.remove(&i);
                                        }
                                    });
                                }
                            >
                                <summary class="templates-pane__summary">
                                    <span class="templates-pane__pattern">
                                        <PatternSpans pattern=pattern/>
                                    </span>
                                    <span class="templates-pane__count">{format!("×{count}")}</span>
                                </summary>
                                <div class="templates-pane__body">
                                    <ExamplesTable examples=examples/>
                                </div>
                            </details>
                        }
                    }
                />
            </div>
        </div>
    }
}

#[component]
fn PatternSpans(pattern: String) -> impl IntoView {
    let parts = pattern_parts(&pattern);
    view! {
        {parts
            .into_iter()
            .map(|(text, is_ph)| {
                if is_ph {
                    view! { <span class="templates-pane__ph">{text}</span> }.into_any()
                } else {
                    view! { <span>{text}</span> }.into_any()
                }
            })
            .collect_view()}
    }
}

fn pattern_parts(pattern: &str) -> Vec<(String, bool)> {
    let bytes = pattern.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut plain_start = 0;
    while i < bytes.len() {
        if bytes[i] == b'['
            && let Some(rel) = pattern[i..].find(']')
        {
            let end = i + rel + 1;
            if plain_start < i {
                out.push((pattern[plain_start..i].to_string(), false));
            }
            out.push((pattern[i..end].to_string(), true));
            i = end;
            plain_start = end;
            continue;
        }
        i += 1;
    }
    if plain_start < pattern.len() {
        out.push((pattern[plain_start..].to_string(), false));
    }
    if out.is_empty() {
        out.push((pattern.to_string(), false));
    }
    out
}

#[component]
fn ExamplesTable(examples: BTreeMap<String, Vec<String>>) -> impl IntoView {
    if examples.is_empty() {
        return view! { <p class="templates-pane__empty">"(no examples)"</p> }.into_any();
    }
    view! {
        <table class="templates-pane__table">
            <thead>
                <tr>
                    <th>"placeholder"</th>
                    <th>"values"</th>
                </tr>
            </thead>
            <tbody>
                {examples
                    .into_iter()
                    .map(|(ph, vals)| {
                        let joined = vals.join(", ");
                        view! {
                            <tr>
                                <td class="templates-pane__ph">{ph}</td>
                                <td>{joined}</td>
                            </tr>
                        }
                    })
                    .collect_view()}
            </tbody>
        </table>
    }
    .into_any()
}
