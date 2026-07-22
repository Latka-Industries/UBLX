//! Bordered 3-pane layout (ratatui `Block`-style) and right-pane tabs.

use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PaneFocus {
    Left,
    Middle,
    Right,
}

/// Shared 3-pane TUI layout — bordered boxes with title nodes.
#[component]
pub(crate) fn ThreePane(
    left_title: &'static str,
    middle_title: &'static str,
    left: AnyView,
    middle: AnyView,
    right: AnyView,
) -> impl IntoView {
    let (focus, set_focus) = signal(PaneFocus::Middle);

    view! {
        <div class="three-pane">
            <PanelBox
                title=left_title
                focused=Signal::derive(move || focus.get() == PaneFocus::Left)
                on_focus=Callback::new(move |_| set_focus.set(PaneFocus::Left))
            >
                {left}
            </PanelBox>
            <PanelBox
                title=middle_title
                focused=Signal::derive(move || focus.get() == PaneFocus::Middle)
                on_focus=Callback::new(move |_| set_focus.set(PaneFocus::Middle))
            >
                {middle}
            </PanelBox>
            <PanelBox
                title="Right"
                hide_default_title=true
                focused=Signal::derive(move || focus.get() == PaneFocus::Right)
                on_focus=Callback::new(move |_| set_focus.set(PaneFocus::Right))
            >
                {right}
            </PanelBox>
        </div>
    }
}

#[component]
fn PanelBox(
    title: &'static str,
    focused: Signal<bool>,
    on_focus: Callback<()>,
    #[prop(optional)] hide_default_title: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <section
            class=move || {
                if focused.get() {
                    "panel panel--focused"
                } else {
                    "panel"
                }
            }
            on:mousedown=move |_| on_focus.run(())
        >
            <div class="panel-frame">
                <Show when=move || !hide_default_title>
                    <div class="panel-titlebar">
                        <span class=move || {
                            if focused.get() {
                                "tab-node tab-node--active tab-node--sm"
                            } else {
                                "tab-node tab-node--sm"
                            }
                        }>{title}</span>
                    </div>
                </Show>
                <div class="panel-inner">{children()}</div>
            </div>
        </section>
    }
}

#[component]
pub(crate) fn PanelRow(
    label: String,
    selected: Signal<bool>,
    on_select: Callback<()>,
) -> impl IntoView {
    view! {
        <li>
            <button
                type="button"
                class=move || {
                    if selected.get() {
                        "panel-row panel-row--selected"
                    } else {
                        "panel-row"
                    }
                }
                on:click=move |_| on_select.run(())
            >
                <span class="panel-row__sym">{move || if selected.get() { "›" } else { " " }}</span>
                <span class="panel-row__text">{label.clone()}</span>
            </button>
        </li>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RightTab {
    Viewer,
    Templates,
    Metadata,
    Writing,
}

impl RightTab {
    fn label(self) -> &'static str {
        match self {
            Self::Viewer => "Viewer",
            Self::Templates => "Templates",
            Self::Metadata => "Metadata",
            Self::Writing => "Writing",
        }
    }
}

#[component]
pub(crate) fn RightPaneShell(#[prop(optional)] overview_only: bool) -> impl IntoView {
    let (tab, set_tab) = signal(RightTab::Viewer);

    view! {
        <div class="right-pane">
            <Show
                when=move || !overview_only
                fallback=move || {
                    view! {
                        <div class="panel-titlebar">
                            <span class="tab-node tab-node--active tab-node--sm">"Overview"</span>
                        </div>
                        <div class="panel-pad pane-empty">
                            "Delta overview — mirrors TUI right pane."
                        </div>
                    }
                }
            >
                <div class="panel-titlebar right-pane-chrome">
                    <nav class="right-tabs" aria-label="Right pane">
                        <RightTabBtn tab=RightTab::Viewer current=tab set=set_tab/>
                        <RightTabBtn tab=RightTab::Templates current=tab set=set_tab/>
                        <RightTabBtn tab=RightTab::Metadata current=tab set=set_tab/>
                        <RightTabBtn tab=RightTab::Writing current=tab set=set_tab/>
                    </nav>
                </div>
                <div class="panel-pad">
                    {move || match tab.get() {
                        RightTab::Viewer => view! {
                            <p class="pane-empty">"Select a path in Contents"</p>
                        }.into_any(),
                        RightTab::Templates => view! {
                            <p class="pane-empty">"Templates"</p>
                        }.into_any(),
                        RightTab::Metadata => view! {
                            <p class="pane-empty">"Metadata"</p>
                        }.into_any(),
                        RightTab::Writing => view! {
                            <p class="pane-empty">"Writing"</p>
                        }.into_any(),
                    }}
                </div>
            </Show>
        </div>
    }
}

#[component]
fn RightTabBtn(
    tab: RightTab,
    current: ReadSignal<RightTab>,
    set: WriteSignal<RightTab>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || {
                if current.get() == tab {
                    "tab-node tab-node--active tab-node--sm"
                } else {
                    "tab-node tab-node--sm"
                }
            }
            on:click=move |_| set.set(tab)
        >
            {tab.label()}
        </button>
    }
}
