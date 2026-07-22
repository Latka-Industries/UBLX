//! Catalog search (TUI `/`) — status strip + client-side Skim fuzzy filter.

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use leptos::prelude::*;

/// Label matches TUI `UI_STRINGS.search.search_label`.
pub(crate) const SEARCH_LABEL: &str = "Search (Categories & Contents): ";

const NO_MATCHES: &str = "(no matches)";

#[derive(Clone, Copy)]
pub(crate) struct CatalogSearch {
    pub query: ReadSignal<String>,
    pub set_query: WriteSignal<String>,
    pub active: ReadSignal<bool>,
    pub set_active: WriteSignal<bool>,
    /// Trimmed query for filters.
    pub trimmed: Signal<String>,
    /// TUI: show strip when typing **or** query still non-empty after Enter.
    pub strip_visible: Signal<bool>,
}

impl CatalogSearch {
    pub(crate) fn provide() -> Self {
        let (query, set_query) = signal(String::new());
        let (active, set_active) = signal(false);
        let trimmed = Signal::derive(move || query.get().trim().to_string());
        let strip_visible = Signal::derive(move || active.get() || !query.get().trim().is_empty());
        let ctx = Self {
            query,
            set_query,
            active,
            set_active,
            trimmed,
            strip_visible,
        };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn clear(self) {
        self.set_query.set(String::new());
        self.set_active.set(false);
    }

    pub(crate) fn submit(self) {
        self.set_active.set(false);
    }

    pub(crate) fn start(self) {
        self.set_active.set(true);
    }
}

fn trim_q(q: &str) -> &str {
    q.trim()
}

fn fuzzy_score(haystack: &str, needle: &str) -> Option<i64> {
    let q = trim_q(needle);
    if q.is_empty() {
        return None;
    }
    SkimMatcherV2::default().fuzzy_match(haystack, q)
}

fn sort_scored(mut scored: Vec<(i64, String)>) -> Vec<String> {
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, s)| s).collect()
}

/// Idle empty-pane copy, or `(no matches)` while a search query is active.
#[must_use]
pub(crate) fn empty_list_message(search_query: &str, idle: &'static str) -> &'static str {
    if trim_q(search_query).is_empty() {
        idle
    } else {
        NO_MATCHES
    }
}

/// Middle-pane `(label, key)` pairs where both are the path.
#[must_use]
pub(crate) fn path_rows(paths: impl IntoIterator<Item = String>) -> Vec<(String, String)> {
    paths.into_iter().map(|p| (p.clone(), p)).collect()
}

/// Best fuzzy score for path or category (TUI `row_fuzzy_score`).
#[must_use]
pub(crate) fn row_fuzzy_score(path: &str, category: &str, needle: &str) -> Option<i64> {
    match (fuzzy_score(path, needle), fuzzy_score(category, needle)) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

#[must_use]
pub(crate) fn fuzzy_matches_field(haystack: &str, needle: &str) -> bool {
    let q = trim_q(needle);
    if q.is_empty() {
        return true;
    }
    fuzzy_score(haystack, q).is_some()
}

/// Labels kept when they fuzzy-match (lens names, etc.).
#[must_use]
pub(crate) fn filter_labels(labels: &[String], search_query: &str) -> Vec<String> {
    let q = trim_q(search_query);
    if q.is_empty() {
        return labels.to_vec();
    }
    labels
        .iter()
        .filter(|l| fuzzy_matches_field(l, q))
        .cloned()
        .collect()
}

/// Categories kept when label matches or any row in that category matches.
#[must_use]
pub(crate) fn filter_categories(
    categories: &[String],
    rows: &[(String, String)],
    search_query: &str,
) -> Vec<String> {
    let q = trim_q(search_query);
    if q.is_empty() {
        return categories.to_vec();
    }
    categories
        .iter()
        .filter(|cat| {
            fuzzy_matches_field(cat, q)
                || rows
                    .iter()
                    .any(|(path, c)| c == *cat && row_fuzzy_score(path, c, q).is_some())
        })
        .cloned()
        .collect()
}

/// Snapshot contents: category gate + fuzzy; non-empty search sorts by score desc, path asc.
#[must_use]
pub(crate) fn filter_snapshot_paths(
    rows: &[(String, String)],
    selected_category: Option<&str>,
    search_query: &str,
) -> Vec<String> {
    let q = trim_q(search_query);
    let in_cat = |cat: &str| selected_category.is_none_or(|c| cat == c);

    if q.is_empty() {
        return rows
            .iter()
            .filter(|(_, cat)| in_cat(cat))
            .map(|(path, _)| path.clone())
            .collect();
    }

    let scored: Vec<(i64, String)> = rows
        .iter()
        .filter(|(_, cat)| in_cat(cat))
        .filter_map(|(path, cat)| row_fuzzy_score(path, cat, q).map(|score| (score, path.clone())))
        .collect();
    sort_scored(scored)
}

/// Path-only fuzzy filter (Delta / Lenses / Duplicates members).
#[must_use]
pub(crate) fn filter_paths(paths: &[String], search_query: &str) -> Vec<String> {
    let q = trim_q(search_query);
    if q.is_empty() {
        return paths.to_vec();
    }
    let scored: Vec<(i64, String)> = paths
        .iter()
        .filter_map(|p| fuzzy_score(p, q).map(|score| (score, p.clone())))
        .collect();
    sort_scored(scored)
}
