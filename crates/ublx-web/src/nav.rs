//! Main-mode navigation for the web chrome.
//!
//! TUI modes are tabs, not URLs. The browser stays on `/` (optional `?mode=`) and
//! switches an in-app [`MainMode`] signal. JSON still lives at serve paths
//! (`/delta`, `/lenses`, …) via [`crate::api`] — never treat those as UI pages.

use leptos::prelude::*;

/// Top-level chrome modes — same set / order idea as TUI `MainMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MainMode {
    #[default]
    Snapshot,
    Lenses,
    Delta,
    Duplicates,
    Settings,
}

impl MainMode {
    pub(crate) const ALL: [Self; 5] = [
        Self::Snapshot,
        Self::Lenses,
        Self::Delta,
        Self::Duplicates,
        Self::Settings,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Snapshot => "Snapshot",
            Self::Lenses => "Lenses",
            Self::Delta => "Delta",
            Self::Duplicates => "Duplicates",
            Self::Settings => "Settings",
        }
    }

    /// Hotkey digit — matches TUI `UblxTabNumber::DEFAULT`.
    pub(crate) fn digit(self) -> u8 {
        match self {
            Self::Snapshot => 1,
            Self::Lenses => 2,
            Self::Delta => 7,
            Self::Duplicates => 8,
            Self::Settings => 9,
        }
    }

    /// Tab bar title with digit hint, e.g. `Settings (9)`.
    pub(crate) fn tab_title(self) -> String {
        format!("{} ({})", self.label(), self.digit())
    }

    /// Query value for `/?mode=…` (never a path segment).
    pub(crate) fn query_value(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Lenses => "lenses",
            Self::Delta => "delta",
            Self::Duplicates => "duplicates",
            Self::Settings => "settings",
        }
    }

    pub(crate) fn from_query_value(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "snapshot" | "" => Some(Self::Snapshot),
            "lenses" | "lens" => Some(Self::Lenses),
            "delta" => Some(Self::Delta),
            "duplicates" | "dupes" | "duplicate" => Some(Self::Duplicates),
            "settings" | "config" => Some(Self::Settings),
            _ => None,
        }
    }

    /// Whether this tab should show given catalog flags (TUI hides empty Lenses/Duplicates).
    pub(crate) fn is_visible(
        self,
        has_lenses: bool,
        has_delta: bool,
        has_duplicates: bool,
    ) -> bool {
        match self {
            Self::Snapshot | Self::Settings => true,
            Self::Lenses => has_lenses,
            // TUI always shows Delta (placeholder if empty); web hides the tab when empty.
            Self::Delta => has_delta,
            Self::Duplicates => has_duplicates,
        }
    }
}

/// First path segment of serve JSON routes — do **not** use these as UI page paths.
pub(crate) const RESERVED_API_PATH_SEGMENTS: &[&str] = &[
    "health",
    "roots",
    "doctor",
    "snapshot",
    "categories",
    "entries",
    "delta",
    "duplicates",
    "lenses",
    "settings",
];

/// True if `segment` is reserved for the JSON API (case-insensitive).
#[must_use]
pub(crate) fn is_reserved_api_segment(segment: &str) -> bool {
    let s = segment.trim().trim_matches('/');
    RESERVED_API_PATH_SEGMENTS
        .iter()
        .any(|r| s.eq_ignore_ascii_case(r))
}

/// True when the pathname is safe for the SPA (not an API route segment).
#[must_use]
pub(crate) fn ui_pathname_is_safe(pathname: &str) -> bool {
    let seg = pathname
        .trim()
        .trim_matches('/')
        .split('/')
        .next()
        .unwrap_or("");
    seg.is_empty() || !is_reserved_api_segment(seg)
}

/// If the address bar is on an API path (e.g. accidental `/delta`), map it to `/?mode=…`
/// and rewrite to `/` so the next refresh still loads the SPA.
pub(crate) fn recover_mode_from_api_pathname() -> Option<MainMode> {
    let window = web_sys::window()?;
    let path = window.location().pathname().ok()?;
    if ui_pathname_is_safe(&path) {
        return None;
    }
    let seg = path.trim_matches('/').split('/').next().unwrap_or("");
    let mode = MainMode::from_query_value(seg).unwrap_or(MainMode::Snapshot);
    let url = if mode == MainMode::Snapshot {
        "/".to_string()
    } else {
        format!("/?mode={}", mode.query_value())
    };
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url));
    }
    Some(mode)
}

/// Read `/?mode=` from the current location (defaults to Snapshot).
pub(crate) fn mode_from_location() -> MainMode {
    if let Some(recovered) = recover_mode_from_api_pathname() {
        return recovered;
    }
    let Some(window) = web_sys::window() else {
        return MainMode::Snapshot;
    };
    let Ok(search) = window.location().search() else {
        return MainMode::Snapshot;
    };
    let raw = search.trim_start_matches('?');
    for pair in raw.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let val = parts.next().unwrap_or("");
        if key == "mode" {
            let decoded = urlencoding::decode(val)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| val.to_string());
            return MainMode::from_query_value(&decoded).unwrap_or(MainMode::Snapshot);
        }
    }
    MainMode::Snapshot
}

/// Update the address bar to `/?mode=…` without a navigation (History API).
pub(crate) fn sync_mode_query(mode: MainMode) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(path) = window.location().pathname() else {
        return;
    };
    let url = if mode == MainMode::Snapshot {
        path
    } else {
        format!("{}?mode={}", path, mode.query_value())
    };
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url));
    }
}

/// Create mode signal seeded from `/?mode=` and keep the query in sync on change.
pub(crate) fn use_main_mode() -> (ReadSignal<MainMode>, WriteSignal<MainMode>) {
    let (mode, set_mode) = signal(mode_from_location());

    Effect::new(move |_| {
        sync_mode_query(mode.get());
    });

    (mode, set_mode)
}

/// Clamp to a visible mode when catalog flags arrive (e.g. `?mode=lenses` with no lenses).
pub(crate) fn clamp_mode_to_visible(
    mode: MainMode,
    has_lenses: bool,
    has_delta: bool,
    has_duplicates: bool,
) -> MainMode {
    if mode.is_visible(has_lenses, has_delta, has_duplicates) {
        mode
    } else {
        MainMode::Snapshot
    }
}

/// Switch mode + sync query. Prefer this over writing the signal alone.
pub(crate) fn select_mode(set_mode: WriteSignal<MainMode>, next: MainMode) {
    set_mode.set(next);
    sync_mode_query(next);
}
