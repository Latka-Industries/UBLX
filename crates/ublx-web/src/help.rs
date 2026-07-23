//! Mode-aware `?` help overlay — TUI-shaped sections, web-relevant bindings only.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::nav::MainMode;

#[derive(Clone, Copy)]
pub(crate) struct HelpOverlay {
    pub visible: ReadSignal<bool>,
    pub set_visible: WriteSignal<bool>,
    pub section: ReadSignal<usize>,
    pub set_section: WriteSignal<usize>,
}

impl HelpOverlay {
    pub(crate) fn provide() -> Self {
        let (visible, set_visible) = signal(false);
        let (section, set_section) = signal(0usize);
        let ctx = Self {
            visible,
            set_visible,
            section,
            set_section,
        };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn open(self) {
        self.set_section.set(0);
        self.set_visible.set(true);
    }

    pub(crate) fn close(self) {
        self.set_visible.set(false);
    }

    pub(crate) fn toggle(self) {
        if self.visible.get_untracked() {
            self.close();
        } else {
            self.open();
        }
    }

    pub(crate) fn cycle_section(self, mode: MainMode, delta: i32) {
        let n = sections_for(mode).len() as i32;
        if n == 0 {
            return;
        }
        let cur = self.section.get_untracked() as i32;
        let next = ((cur + delta).rem_euclid(n)) as usize;
        self.set_section.set(next);
    }
}

#[derive(Clone, Copy)]
struct HelpSection {
    title: &'static str,
    rows: &'static [(&'static str, &'static str)],
    include_digit_row: bool,
}

const DIGIT_ROW_KEY: &str = "1 2 7 8 9";
const DIGIT_ROW_DESC: &str = "Jump to Main Tab when that tab is visible.";

const FOOTNOTE: &str =
    "Only bindings that work in the web UI are listed. Find and menus land in later PRs.";

const GENERAL_BROWSER: &[(&str, &str)] = &[
    ("~", "Alternate between Main tabs"),
    ("/", "Fuzzy catalog filter; Enter (apply) · Esc (clear)"),
    (
        "Tab",
        "Switch left or middle pane focus (categories ↔ contents)",
    ),
    ("h | l · ← →", "Focus Left or Middle pane"),
    ("j | k · ↑ ↓", "Move down / up in focused pane"),
    ("g | G", "Go to top / bottom of focused list"),
    ("Ctrl+j/k · Ctrl+↑↓", "Jump by 10 in focused list"),
    ("s", "Cycle middle-pane sort (Snapshot / Delta / Dupes)"),
    ("?", "Toggle this help"),
];

const RIGHT_PANE: &[(&str, &str)] = &[
    (
        "v / t / m / w",
        "Viewer / Templates / Metadata / Writing (when available)",
    ),
    ("Shift+Tab", "Cycle right-pane tabs"),
    (
        "Shift+J/↓ · Shift+K/↑",
        "Scroll right pane (or PDF: next / previous page)",
    ),
    (
        "Shift+B · Shift+E",
        "Jump right pane to top / bottom (or PDF: first / last page)",
    ),
    (
        "Shift+S",
        "Viewer find; Enter apply · Shift+S re-edit · n/N next/prev · Esc clear",
    ),
];

const GENERAL_SETTINGS: &[(&str, &str)] = &[
    ("h | l · ← →", "Focus Scope or Options pane"),
    ("j | k · ↑ ↓", "Move in the focused list (Scope / Options)"),
    ("Tab", "Cycle left ↔ middle pane focus"),
    ("?", "Toggle this help"),
];

const SNAPSHOT_SECTIONS: &[HelpSection] = &[
    HelpSection {
        title: "General",
        rows: GENERAL_BROWSER,
        include_digit_row: true,
    },
    HelpSection {
        title: "Right Pane",
        rows: RIGHT_PANE,
        include_digit_row: false,
    },
];

const DELTA_SECTIONS: &[HelpSection] = &[HelpSection {
    title: "General",
    rows: GENERAL_BROWSER,
    include_digit_row: true,
}];

const SETTINGS_SECTIONS: &[HelpSection] = &[HelpSection {
    title: "General",
    rows: GENERAL_SETTINGS,
    include_digit_row: true,
}];

fn sections_for(mode: MainMode) -> &'static [HelpSection] {
    match mode {
        MainMode::Snapshot | MainMode::Lenses | MainMode::Duplicates => SNAPSHOT_SECTIONS,
        MainMode::Delta => DELTA_SECTIONS,
        MainMode::Settings => SETTINGS_SECTIONS,
    }
}

fn context_blurb(mode: MainMode) -> &'static str {
    match mode {
        MainMode::Snapshot => "Current Mode: category tree, file list, and right-pane viewer.",
        MainMode::Delta => "Current Mode: snapshot overview and added / modified / removed lists.",
        MainMode::Lenses => "Current Mode: lens names, member paths, and right-pane viewer.",
        MainMode::Duplicates => {
            "Current Mode: duplicate groups, member paths, and right-pane viewer."
        }
        MainMode::Settings => "Current Mode: edit Global or Local settings.",
    }
}

#[component]
pub(crate) fn HelpModal(mode: ReadSignal<MainMode>) -> impl IntoView {
    let help = HelpOverlay::expect();

    view! {
        <Show when=move || help.visible.get()>
            <div
                class="help-overlay"
                role="dialog"
                aria-modal="true"
                aria-label="Keyboard help"
                on:mousedown=move |ev| {
                    if let Some(t) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                        && t.class_list().contains("help-overlay")
                    {
                        help.close();
                    }
                }
            >
                <div class="help-panel" on:mousedown=move |ev| ev.stop_propagation()>
                    {move || {
                        let m = mode.get();
                        let secs = sections_for(m);
                        let idx = help.section.get().min(secs.len().saturating_sub(1));
                        let sec = secs.get(idx).copied().unwrap_or(secs[0]);
                        view! {
                            <div class="help-header">
                                <span class="help-title">"Help"</span>
                                <span class="help-hint">"Tab / ← → cycle · Esc or ? close"</span>
                            </div>
                            <p class="help-blurb">{context_blurb(m)}</p>
                            <nav class="help-tabs" aria-label="Help sections">
                                {secs
                                    .iter()
                                    .enumerate()
                                    .map(|(i, s)| {
                                        let title = s.title;
                                        view! {
                                            <button
                                                type="button"
                                                class=move || {
                                                    if help.section.get() == i {
                                                        "tab-node tab-node--active tab-node--sm"
                                                    } else {
                                                        "tab-node tab-node--sm"
                                                    }
                                                }
                                                on:mousedown=move |ev| ev.prevent_default()
                                                on:click=move |_| help.set_section.set(i)
                                            >
                                                {title}
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </nav>
                            <div class="help-table-wrap">
                                <table class="help-table">
                                    <thead>
                                        <tr>
                                            <th>"Keys"</th>
                                            <th>"Action"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <Show when=move || sec.include_digit_row>
                                            <tr>
                                                <td class="help-key">{DIGIT_ROW_KEY}</td>
                                                <td>{DIGIT_ROW_DESC}</td>
                                            </tr>
                                        </Show>
                                        {sec
                                            .rows
                                            .iter()
                                            .map(|(k, d)| {
                                                view! {
                                                    <tr>
                                                        <td class="help-key">{*k}</td>
                                                        <td>{*d}</td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()}
                                    </tbody>
                                </table>
                            </div>
                            <p class="help-footnote">{FOOTNOTE}</p>
                        }
                    }}
                </div>
            </div>
        </Show>
    }
}
