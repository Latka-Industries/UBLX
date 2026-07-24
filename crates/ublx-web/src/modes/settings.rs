//! Settings: Global/Local · control rows · live read-only TOML (no text editor).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{
    SettingsLayoutPatch, SettingsPatch, SettingsScope, SettingsView, fetch_settings, patch_settings,
};
use crate::command_mode::CommandModeCtx;
use crate::focus::{ListNav, UiNav, install_list_nav};
use crate::panes::{PanelRow, ThreePane, schedule_scroll_selected_into_view};
use crate::theme::apply_theme_css_body;

#[derive(Clone, Debug, PartialEq, Eq)]
enum FocusedOption {
    Bool(String),
    Theme,
    TypedColumnTables,
    LayoutLeft,
    LayoutMiddle,
    LayoutRight,
}

impl FocusedOption {
    fn description<'a>(&self, view: &'a SettingsView) -> &'a str {
        match self {
            Self::Bool(key) => view
                .bools
                .iter()
                .find(|b| &b.key == key)
                .map(|b| b.description.as_str())
                .unwrap_or("Settings option."),
            Self::Theme => "Active color palette (written to this scope's ublx.toml).",
            Self::TypedColumnTables => {
                "none: hide String/Number/… column tables in Metadata; abbrev: cap tables longer than 20 rows to 20 rows; full: show all metadata"
            }
            Self::LayoutLeft => "Left pane width percentage (categories). Sum must be 100.",
            Self::LayoutMiddle => "Middle pane width percentage (contents). Sum must be 100.",
            Self::LayoutRight => "Right pane width percentage (preview). Sum must be 100.",
        }
    }
}

fn cycle_typed_column_tables(current: &str) -> &'static str {
    match current {
        "none" => "abbrev",
        "abbrev" => "full",
        _ => "none",
    }
}

fn bool_patch(key: &str, value: bool) -> SettingsPatch {
    let mut p = SettingsPatch::default();
    match key {
        "show_hidden_files" => p.show_hidden_files = Some(value),
        "hash" => p.hash = Some(value),
        "enable_enhance_all" => p.enable_enhance_all = Some(value),
        "ask_enhance_on_new_root" => p.ask_enhance_on_new_root = Some(value),
        "run_snapshot_on_startup" => p.run_snapshot_on_startup = Some(value),
        _ => {}
    }
    p
}

fn bump_layout(view: &SettingsView, which: FocusedOption, delta: i16) -> Option<SettingsPatch> {
    let mut left = view.layout.left_pct as i16;
    let mut middle = view.layout.middle_pct as i16;
    let mut right = view.layout.right_pct as i16;
    let (target, donor) = match which {
        FocusedOption::LayoutLeft => (&mut left, &mut middle),
        FocusedOption::LayoutMiddle => (&mut middle, &mut right),
        FocusedOption::LayoutRight => (&mut right, &mut middle),
        _ => return None,
    };
    let next = *target + delta;
    let donor_next = *donor - delta;
    if !(5..=90).contains(&next) || !(5..=90).contains(&donor_next) {
        return None;
    }
    *target = next;
    *donor = donor_next;
    Some(SettingsPatch {
        layout: Some(SettingsLayoutPatch {
            left_pct: left as u16,
            middle_pct: middle as u16,
            right_pct: right as u16,
        }),
        ..Default::default()
    })
}

#[component]
pub(crate) fn SettingsMode() -> impl IntoView {
    let (scope, set_scope) = signal(SettingsScope::Local);
    let (live, set_live) = signal(None::<SettingsView>);
    let (focus, set_focus) = signal(None::<FocusedOption>);
    let (err, set_err) = signal(None::<String>);
    let (busy, set_busy) = signal(false);
    let command_mode = CommandModeCtx::expect();

    let loaded = LocalResource::new(move || {
        let s = scope.get();
        let _ = command_mode.theme_committed.get();
        async move { fetch_settings(s).await }
    });

    Effect::new(move |_| {
        let _ = scope.get();
        set_live.set(None);
        set_focus.set(None);
        set_err.set(None);
    });

    Effect::new(move |_| {
        if let Some(res) = loaded.get() {
            match res {
                Ok(v) => {
                    apply_theme_css_body(&v.css);
                    if !v.theme.is_empty() {
                        command_mode.highlight_theme.set(v.theme.clone());
                    }
                    set_live.set(Some(v));
                    set_err.set(None);
                }
                Err(e) => set_err.set(Some(e)),
            }
        }
    });

    let apply = Callback::new(move |patch: SettingsPatch| {
        if busy.get_untracked() {
            return;
        }
        let s = scope.get_untracked();
        set_busy.set(true);
        set_err.set(None);
        spawn_local(async move {
            match patch_settings(s, &patch).await {
                Ok(v) => {
                    apply_theme_css_body(&v.css);
                    if !v.theme.is_empty() {
                        command_mode.highlight_theme.set(v.theme.clone());
                    }
                    set_live.set(Some(v));
                    set_err.set(None);
                }
                Err(e) => set_err.set(Some(e)),
            }
            set_busy.set(false);
        });
    });

    let view_sig = Signal::derive(move || live.get());

    let nav = UiNav::expect();
    let scopes = [SettingsScope::Global, SettingsScope::Local];
    install_list_nav(
        nav.left,
        ListNav {
            move_by: Callback::new(move |delta: i32| {
                let idx = scopes
                    .iter()
                    .position(|s| *s == scope.get_untracked())
                    .unwrap_or(0);
                let n = scopes.len() as i32;
                let next = ((idx as i32 + delta).clamp(0, n - 1)) as usize;
                set_scope.set(scopes[next]);
            }),
            to_start: Callback::new(move |_| set_scope.set(scopes[0])),
            to_end: Callback::new(move |_| set_scope.set(scopes[1])),
        },
    );

    view! {
        <ThreePane
            left_title="Scope"
            middle_title="Options"
            left=view! {
                <ul class="panel-list">
                    {[SettingsScope::Global, SettingsScope::Local]
                        .into_iter()
                        .map(|s| {
                            view! {
                                <PanelRow
                                    label=s.label().to_string()
                                    selected=Signal::derive(move || scope.get() == s)
                                    on_select=Callback::new(move |_| set_scope.set(s))
                                />
                            }
                        })
                        .collect_view()}
                </ul>
            }
            .into_any()
            middle=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    {move || {
                        let Some(v) = view_sig.get() else {
                            if let Some(e) = err.get() {
                                return view! { <p class="pane-empty">{e}</p> }.into_any();
                            }
                            return view! { <p class="pane-empty">"…"</p> }.into_any();
                        };
                        let path_hint = v.path.clone();
                        let path_title = path_hint.clone();
                        let exists = v.exists;
                        let scroll_ref = NodeRef::<leptos::html::Div>::new();
                        Effect::new(move |_| {
                            let _ = focus.get();
                            let Some(scroll) = scroll_ref.get() else {
                                return;
                            };
                            schedule_scroll_selected_into_view(scroll.into());
                        });
                        view! {
                            <div class="paths-pane">
                                <div class="panel-scroll" node_ref=scroll_ref>
                                    <Show when=move || err.get().is_some()>
                                        <p class="pane-empty settings-err">
                                            {move || err.get().unwrap_or_default()}
                                        </p>
                                    </Show>
                                    <ul class="panel-list">
                                        {v.bools
                                            .iter()
                                            .map(|b| {
                                                let key = b.key.clone();
                                                let key2 = key.clone();
                                                let key3 = key.clone();
                                                let label = format!(
                                                    "{} — {}",
                                                    b.label,
                                                    if b.value { "on" } else { "off" }
                                                );
                                                let next = !b.value;
                                                view! {
                                                    <PanelRow
                                                        label=label
                                                        selected=Signal::derive(move || {
                                                            focus.get()
                                                                == Some(FocusedOption::Bool(
                                                                    key.clone(),
                                                                ))
                                                        })
                                                        on_select=Callback::new(move |_| {
                                                            set_focus.set(Some(
                                                                FocusedOption::Bool(key2.clone()),
                                                            ));
                                                            apply.run(bool_patch(&key3, next));
                                                        })
                                                    />
                                                }
                                            })
                                            .collect_view()}
                                        <li class="settings-divider"/>
                                        <li
                                            class=move || {
                                                if focus.get() == Some(FocusedOption::Theme) {
                                                    "settings-inline-row settings-inline-row--selected"
                                                } else {
                                                    "settings-inline-row"
                                                }
                                            }
                                            on:mousedown=move |_| {
                                                set_focus.set(Some(FocusedOption::Theme));
                                            }
                                        >
                                            <span class="settings-inline-row__label">"theme"</span>
                                            <button
                                                type="button"
                                                class="settings-select settings-theme-trigger"
                                                prop:disabled=move || busy.get()
                                                on:mousedown=move |ev| {
                                                    // Keep focus on the row; avoid stealing from the shell keybus.
                                                    ev.prevent_default();
                                                }
                                                on:click=move |_| {
                                                    set_focus.set(Some(FocusedOption::Theme));
                                                    command_mode
                                                        .open_theme_selector(scope.get_untracked());
                                                }
                                            >
                                                {move || {
                                                    live.get().map(|cur| cur.theme).unwrap_or_default()
                                                }}
                                            </button>
                                        </li>
                                        <li
                                            class=move || {
                                                if focus.get() == Some(FocusedOption::TypedColumnTables)
                                                {
                                                    "settings-inline-row settings-inline-row--selected"
                                                } else {
                                                    "settings-inline-row"
                                                }
                                            }
                                            on:mousedown=move |_| {
                                                set_focus.set(Some(FocusedOption::TypedColumnTables));
                                            }
                                            on:click=move |_| {
                                                let cur = view_sig
                                                    .get()
                                                    .map(|v| v.typed_column_tables.clone())
                                                    .unwrap_or_else(|| "abbrev".into());
                                                let next = cycle_typed_column_tables(&cur);
                                                apply.run(SettingsPatch {
                                                    typed_column_tables: Some(next.into()),
                                                    ..Default::default()
                                                });
                                            }
                                        >
                                            <span class="settings-inline-row__label">
                                                "typed_column_tables"
                                            </span>
                                            <span class="settings-inline-row__value">
                                                {move || {
                                                    view_sig
                                                        .get()
                                                        .map(|v| v.typed_column_tables.clone())
                                                        .unwrap_or_else(|| "abbrev".into())
                                                }}
                                            </span>
                                        </li>
                                        <li class="settings-divider"/>
                                        {layout_row(
                                            "layout left%",
                                            v.layout.left_pct,
                                            FocusedOption::LayoutLeft,
                                            focus,
                                            set_focus,
                                            view_sig,
                                            apply,
                                            busy,
                                        )}
                                        {layout_row(
                                            "layout middle%",
                                            v.layout.middle_pct,
                                            FocusedOption::LayoutMiddle,
                                            focus,
                                            set_focus,
                                            view_sig,
                                            apply,
                                            busy,
                                        )}
                                        {layout_row(
                                            "layout right%",
                                            v.layout.right_pct,
                                            FocusedOption::LayoutRight,
                                            focus,
                                            set_focus,
                                            view_sig,
                                            apply,
                                            busy,
                                        )}
                                    </ul>
                                </div>
                                <div class="pane-footer pane-footer--start">
                                    <span class="status-node">
                                        {if exists { "file" } else { "new" }}
                                    </span>
                                    <span class="status-node" title=path_title>
                                        {path_hint}
                                    </span>
                                    <Show when=move || busy.get()>
                                        <span class="status-node">"saving…"</span>
                                    </Show>
                                </div>
                            </div>
                        }
                        .into_any()
                    }}
                </Suspense>
            }
            .into_any()
            right=view! {
                <div class="right-pane">
                    <div class="panel-titlebar right-pane-chrome">
                        <span class="tab-node tab-node--active tab-node--sm">"TOML"</span>
                        <span class="tab-node tab-node--sm">"(read-only)"</span>
                    </div>
                    <div class="panel-pad settings-toml-pane">
                        {move || {
                            let Some(v) = view_sig.get() else {
                                return view! { <p class="pane-empty">"…"</p> }.into_any();
                            };
                            let desc = focus
                                .get()
                                .map(|f| f.description(&v).to_string())
                                .unwrap_or_else(|| {
                                    "Toggle or step Options. Live TOML below updates after each write."
                                        .into()
                                });
                            let toml = if v.toml.is_empty() {
                                "# (file missing — first write creates it)\n".to_string()
                            } else {
                                v.toml
                            };
                            view! {
                                <p class="settings-desc">{desc}</p>
                                <pre class="detail-pre settings-toml">{toml}</pre>
                            }
                            .into_any()
                        }}
                    </div>
                </div>
            }
            .into_any()
        />
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_row(
    label: &'static str,
    pct: u16,
    which: FocusedOption,
    focus: ReadSignal<Option<FocusedOption>>,
    set_focus: WriteSignal<Option<FocusedOption>>,
    view_sig: Signal<Option<SettingsView>>,
    apply: Callback<SettingsPatch>,
    busy: ReadSignal<bool>,
) -> AnyView {
    let which_sel = which.clone();
    let which_focus = which.clone();
    let which_minus = which.clone();
    let which_plus = which.clone();
    view! {
        <PanelRow
            label=format!("{label} — {pct}")
            selected=Signal::derive(move || focus.get() == Some(which_sel.clone()))
            on_select=Callback::new(move |_| set_focus.set(Some(which_focus.clone())))
        />
        <li class="settings-stepper">
            <button
                type="button"
                class="settings-step"
                prop:disabled=move || busy.get()
                on:click=move |_| {
                    set_focus.set(Some(which_minus.clone()));
                    if let Some(v) = view_sig.get_untracked()
                        && let Some(p) = bump_layout(&v, which_minus.clone(), -1)
                    {
                        apply.run(p);
                    }
                }
            >
                "−"
            </button>
            <button
                type="button"
                class="settings-step"
                prop:disabled=move || busy.get()
                on:click=move |_| {
                    set_focus.set(Some(which_plus.clone()));
                    if let Some(v) = view_sig.get_untracked()
                        && let Some(p) = bump_layout(&v, which_plus.clone(), 1)
                    {
                        apply.run(p);
                    }
                }
            >
                "+"
            </button>
        </li>
    }
    .into_any()
}
