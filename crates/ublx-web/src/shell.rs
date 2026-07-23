//! App chrome: main tabs, project path, catalog search / Last Snapshot footer.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

use crate::api::{CatalogFlags, format_timestamp_ns};
use crate::catalog_refresh::CatalogRefresh;
use crate::command_mode::{CommandModeCtx, CommandModePopup};
use crate::focus::{PaneFocus, PdfPageNav, PreviewKeysBus, RightTabBus, UiNav};
use crate::help::{HelpModal, HelpOverlay};
use crate::keys::{
    CommandModeKeyCtx, FindKeyCtx, MultiselectKeyCtx, SpaceMenuKeyCtx, WebAction,
    action_from_keydown, typing_in_form_field,
};
use crate::modes::{DeltaMode, DuplicatesMode, LensesMode, SettingsMode, SnapshotMode};
use crate::multiselect::MultiselectCtx;
use crate::nav::{MainMode, clamp_mode_to_visible, select_mode, use_main_mode};
use crate::search::{CatalogSearch, SEARCH_LABEL};
use crate::sort::ContentSortCtx;
use crate::space_menu::{SpaceMenuCtx, SpaceMenuPopup};
use crate::toast::{ToastCtx, ToastHost};
use crate::viewer::scroll_right_preview;
use crate::viewer_find::ViewerFind;

#[component]
pub(crate) fn Shell(flags: CatalogFlags) -> impl IntoView {
    let flags = StoredValue::new(flags);
    let (mode, set_mode) = use_main_mode();
    let search = CatalogSearch::provide();
    let find = ViewerFind::provide();
    let sort = ContentSortCtx::provide();
    let (nav, tabs, preview) = UiNav::provide();
    let help = HelpOverlay::provide();
    let multiselect = MultiselectCtx::provide();
    let catalog_refresh = CatalogRefresh::provide();
    let toasts = ToastCtx::provide();
    let space_menu = SpaceMenuCtx::provide(catalog_refresh, multiselect, toasts);
    space_menu.catalog_root.set(flags.get_value().root.clone());
    let command_mode = CommandModeCtx::provide(catalog_refresh, set_mode, toasts);

    // Deep-link may name a tab that is hidden for this catalog — fall back to Snapshot.
    Effect::new(move |_| {
        let f = flags.get_value();
        let clamped =
            clamp_mode_to_visible(mode.get(), f.has_lenses, f.has_delta, f.has_duplicates);
        if clamped != mode.get_untracked() {
            select_mode(set_mode, clamped);
        }
    });

    // Reset help section when main mode changes while open.
    Effect::new(move |_| {
        let _ = mode.get();
        if help.visible.get_untracked() {
            help.set_section.set(0);
        }
    });

    // Multi-select clears on mode switch (TUI `apply_mode_switch`).
    Effect::new(move |_| {
        let _ = mode.get();
        multiselect.clear();
        space_menu.close();
        command_mode.close_all();
    });

    // Leaving contents pane exits multi-select (TUI FocusCategories / Tab).
    Effect::new(move |_| {
        if nav.pane.get() != PaneFocus::Middle && multiselect.active.get_untracked() {
            multiselect.clear();
        }
    });

    // Chord chrome hint while waiting for the second key (before menu).
    Effect::new(move |_| {
        let pending = command_mode.pending.get() && !command_mode.menu_visible.get();
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.query_selector(".tui-shell").ok().flatten())
        {
            let list = el.class_list();
            if pending {
                let _ = list.add_1("tui-shell--chord-pending");
            } else {
                let _ = list.remove_1("tui-shell--chord-pending");
            }
        }
    });

    // Global keybus — TUI-shaped hotkeys (ignore while typing in forms).
    Effect::new(move |_| {
        let Some(window) = web_sys::window() else {
            return;
        };
        let search = search;
        let find = find;
        let sort = sort;
        let nav = nav;
        let tabs = tabs;
        let mode = mode;
        let set_mode = set_mode;
        let flags = flags;
        let help = help;
        let preview = preview;
        let multiselect = multiselect;
        let space_menu = space_menu;
        let command_mode = command_mode;
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |ev: KeyboardEvent| {
            let menu_open = space_menu.visible.get_untracked();
            let cmd_active = command_mode.is_active();
            // Rename / bulk-rename inputs need normal typing; their own on:keydown handles Enter/Esc.
            if typing_in_form_field() && !menu_open && !cmd_active {
                return;
            }
            if typing_in_form_field() && (menu_open || cmd_active) {
                return;
            }
            let help_open = help.visible.get_untracked();
            let find_ctx = FindKeyCtx {
                committed: find.committed.get_untracked() && !find.active.get_untracked(),
                catalog_search_active: search.active.get_untracked(),
                allow: mode.get_untracked() != MainMode::Settings,
            };
            let ms_ctx = MultiselectKeyCtx {
                active: multiselect.active.get_untracked(),
                applies: MultiselectCtx::applies(mode.get_untracked()),
                middle_focused: nav.pane.get_untracked() == PaneFocus::Middle,
            };
            let m = mode.get_untracked();
            let space_ctx = SpaceMenuKeyCtx {
                open: menu_open,
                can_open: matches!(
                    m,
                    MainMode::Snapshot | MainMode::Lenses | MainMode::Duplicates
                ),
            };
            let cmd_ctx = CommandModeKeyCtx {
                active: cmd_active,
                picker: command_mode.picker_open(),
                blocked: help_open
                    || menu_open
                    || search.active.get_untracked()
                    || m == MainMode::Settings,
                leader: command_mode.leader.get_untracked(),
            };
            let Some(action) =
                action_from_keydown(&ev, help_open, find_ctx, ms_ctx, space_ctx, cmd_ctx)
            else {
                return;
            };
            ev.prevent_default();
            // Capture-phase: stop other handlers (focused <button> Enter, etc.) from stealing keys.
            if menu_open || cmd_active {
                ev.stop_immediate_propagation();
            }
            dispatch_action(
                action,
                KeybusCtx {
                    search,
                    find,
                    sort,
                    nav,
                    tabs,
                    preview,
                    mode,
                    set_mode,
                    flags,
                    help,
                    multiselect,
                    space_menu,
                    command_mode,
                },
            );
        }) as Box<dyn FnMut(_)>);
        // Capture so Space-menu Enter/letters win over a focused panel/menu button.
        let _ = window.add_event_listener_with_callback_and_bool(
            "keydown",
            closure.as_ref().unchecked_ref(),
            true,
        );
        closure.forget();
    });

    view! {
        <header class="main-chrome">
            <MainTabBar
                mode=mode
                set_mode=set_mode
                has_lenses=Signal::derive(move || flags.get_value().has_lenses)
                has_delta=Signal::derive(move || flags.get_value().has_delta)
                has_duplicates=Signal::derive(move || flags.get_value().has_duplicates)
            />
            <div class="brand" aria-label="UBLX">"UBLX"</div>
        </header>

        <div class="project-path" title=move || flags.get_value().root.clone().unwrap_or_default()>
            {
                move || {
                    flags
                        .get_value()
                        .root
                        .clone()
                        .unwrap_or_else(|| "—".into())
                }
            }
        </div>

        <main class="mode-body">
            {move || match mode.get() {
                MainMode::Snapshot => view! { <SnapshotMode/> }.into_any(),
                MainMode::Lenses => view! { <LensesMode/> }.into_any(),
                MainMode::Delta => view! { <DeltaMode/> }.into_any(),
                MainMode::Duplicates => view! { <DuplicatesMode/> }.into_any(),
                MainMode::Settings => view! { <SettingsMode/> }.into_any(),
            }}
        </main>

        <footer class="status-chrome">
            <FooterNodes flags=flags search=search/>
        </footer>

        <HelpModal mode=mode/>
        <ToastHost/>
        <SpaceMenuPopup/>
        <CommandModePopup/>
    }
}

#[derive(Clone, Copy)]
struct KeybusCtx {
    search: CatalogSearch,
    find: ViewerFind,
    sort: ContentSortCtx,
    nav: UiNav,
    tabs: RightTabBus,
    preview: PreviewKeysBus,
    mode: ReadSignal<MainMode>,
    set_mode: WriteSignal<MainMode>,
    flags: StoredValue<CatalogFlags>,
    help: HelpOverlay,
    multiselect: MultiselectCtx,
    space_menu: SpaceMenuCtx,
    command_mode: CommandModeCtx,
}

fn dispatch_action(action: WebAction, ctx: KeybusCtx) {
    let f = ctx.flags.get_value();
    match action {
        WebAction::HelpToggle => {
            ctx.space_menu.close();
            ctx.command_mode.close_all();
            ctx.help.toggle();
        }
        WebAction::HelpClose => ctx.help.close(),
        WebAction::HelpSectionNext => ctx.help.cycle_section(ctx.mode.get_untracked(), 1),
        WebAction::HelpSectionPrev => ctx.help.cycle_section(ctx.mode.get_untracked(), -1),
        WebAction::HelpAbsorb => {}
        WebAction::CommandModeBegin => {
            ctx.help.close();
            ctx.space_menu.close();
            ctx.command_mode.begin_chord();
        }
        WebAction::CommandModeClose => ctx.command_mode.close_all(),
        WebAction::CommandModeHotkey(c) => {
            let _ = ctx.command_mode.submit_hotkey(c);
        }
        WebAction::CommandModeMoveUp => ctx.command_mode.picker_move(-1),
        WebAction::CommandModeMoveDown => ctx.command_mode.picker_move(1),
        WebAction::CommandModeSubmit => ctx.command_mode.picker_submit(),
        WebAction::CommandModeAbsorb => {}
        WebAction::SpaceMenuOpen => {
            ctx.command_mode.close_all();
            let _ = ctx.space_menu.try_open_qa(
                ctx.mode.get_untracked(),
                ctx.nav.pane.get_untracked(),
                ctx.multiselect.active.get_untracked(),
            );
        }
        WebAction::SpaceMenuMoveUp => ctx.space_menu.move_sel(-1),
        WebAction::SpaceMenuMoveDown => ctx.space_menu.move_sel(1),
        WebAction::SpaceMenuSubmit => {
            ctx.space_menu
                .submit_selected(ctx.flags.get_value().root.clone());
        }
        WebAction::SpaceMenuClose => ctx.space_menu.close(),
        WebAction::SpaceMenuHotkey(c) => {
            let root = ctx.flags.get_value().root.clone();
            if !ctx.space_menu.submit_hotkey(c, root) {
                // No row for this letter — j/k still move the highlight (arrows always move).
                match c {
                    'k' => ctx.space_menu.move_sel(-1),
                    'j' => ctx.space_menu.move_sel(1),
                    _ => {}
                }
            }
        }
        WebAction::SpaceMenuAbsorb => {}
        WebAction::SearchStart => {
            ctx.find.clear();
            ctx.space_menu.close();
            ctx.command_mode.close_all();
            ctx.search.start();
        }
        WebAction::ViewerFindOpen => {
            ctx.search.set_active.set(false);
            ctx.space_menu.close();
            ctx.command_mode.close_all();
            ctx.find.start();
        }
        WebAction::ViewerFindNext => ctx.find.next(),
        WebAction::ViewerFindPrev => ctx.find.prev(),
        WebAction::ViewerFindClear => ctx.find.clear(),
        WebAction::MultiselectToggleMode => {
            ctx.space_menu.close();
            ctx.command_mode.close_all();
            let _ = ctx.multiselect.try_toggle_mode(
                ctx.mode.get_untracked(),
                ctx.nav.pane.get_untracked() == PaneFocus::Middle,
            );
        }
        WebAction::MultiselectToggleRow => {
            if ctx.nav.pane.get_untracked() == PaneFocus::Middle {
                ctx.multiselect.toggle_row();
            }
        }
        WebAction::MultiselectCancel => {
            ctx.space_menu.close();
            ctx.multiselect.clear();
        }
        WebAction::MultiselectOpenBulk => {
            ctx.command_mode.close_all();
            let _ = ctx.space_menu.try_open_bulk(
                ctx.mode.get_untracked(),
                ctx.multiselect.active.get_untracked(),
            );
        }
        WebAction::CycleContentSort => ctx.sort.cycle(ctx.mode.get_untracked()),
        WebAction::ScrollPreviewDown => apply_preview_keys(ctx.preview, PdfPageNav::Next),
        WebAction::ScrollPreviewUp => apply_preview_keys(ctx.preview, PdfPageNav::Prev),
        WebAction::PreviewTop => apply_preview_keys(ctx.preview, PdfPageNav::Top),
        WebAction::PreviewBottom => apply_preview_keys(ctx.preview, PdfPageNav::Bottom),
        WebAction::MainMode(m) => {
            if m.is_visible(f.has_lenses, f.has_delta, f.has_duplicates) {
                select_mode(ctx.set_mode, m);
            }
        }
        WebAction::MainModeToggle => {
            let next = next_visible_mode(
                ctx.mode.get_untracked(),
                f.has_lenses,
                f.has_delta,
                f.has_duplicates,
            );
            select_mode(ctx.set_mode, next);
        }
        WebAction::FocusLeft => ctx.nav.set_pane.set(PaneFocus::Left),
        WebAction::FocusMiddle => ctx.nav.set_pane.set(PaneFocus::Middle),
        WebAction::FocusCycle => {
            let next = ctx.nav.pane.get_untracked().cycle();
            ctx.nav.set_pane.set(next);
        }
        WebAction::MoveUp => {
            blur_panel_row_focus();
            if let Some(list) = ctx.nav.active_list() {
                list.move_by.run(-1);
            }
        }
        WebAction::MoveDown => {
            blur_panel_row_focus();
            if let Some(list) = ctx.nav.active_list() {
                list.move_by.run(1);
            }
        }
        WebAction::MoveUpFast => {
            blur_panel_row_focus();
            if let Some(list) = ctx.nav.active_list() {
                list.move_by.run(-10);
            }
        }
        WebAction::MoveDownFast => {
            blur_panel_row_focus();
            if let Some(list) = ctx.nav.active_list() {
                list.move_by.run(10);
            }
        }
        WebAction::ListTop => {
            blur_panel_row_focus();
            if let Some(list) = ctx.nav.active_list() {
                list.to_start.run(());
            }
        }
        WebAction::ListBottom => {
            blur_panel_row_focus();
            if let Some(list) = ctx.nav.active_list() {
                list.to_end.run(());
            }
        }
        WebAction::RightTab(t) => {
            ctx.tabs.set_request.set(Some(t));
        }
        WebAction::CycleRightTab => {
            ctx.tabs.bump_cycle.update(|n| *n = n.wrapping_add(1));
        }
    }
}

fn apply_preview_keys(preview: PreviewKeysBus, nav: PdfPageNav) {
    if let Some(ctl) = preview.pdf.get_untracked() {
        ctl.apply.run(nav);
    } else if let Some(ctl) = preview.text_win.get_untracked() {
        ctl.apply.run(nav);
    } else {
        scroll_right_preview(nav);
    }
}

/// Drop DOM focus from a clicked panel row so keyboard selection is the only highlight.
fn blur_panel_row_focus() {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(el) = doc.active_element() else {
        return;
    };
    let class = el.get_attribute("class").unwrap_or_default();
    if class.split_whitespace().any(|c| c == "panel-row")
        && let Ok(html) = el.dyn_into::<web_sys::HtmlElement>()
    {
        let _ = html.blur();
    }
}

fn next_visible_mode(
    current: MainMode,
    has_lenses: bool,
    has_delta: bool,
    has_duplicates: bool,
) -> MainMode {
    let visible: Vec<MainMode> = MainMode::ALL
        .into_iter()
        .filter(|m| m.is_visible(has_lenses, has_delta, has_duplicates))
        .collect();
    if visible.is_empty() {
        return MainMode::Snapshot;
    }
    let idx = visible.iter().position(|m| *m == current).unwrap_or(0);
    visible[(idx + 1) % visible.len()]
}

#[component]
fn MainTabBar(
    mode: ReadSignal<MainMode>,
    set_mode: WriteSignal<MainMode>,
    has_lenses: Signal<bool>,
    has_delta: Signal<bool>,
    has_duplicates: Signal<bool>,
) -> impl IntoView {
    view! {
        <nav class="main-tabs" aria-label="Main modes">
            {MainMode::ALL
                .into_iter()
                .map(|m| {
                    let visible = Signal::derive(move || {
                        m.is_visible(has_lenses.get(), has_delta.get(), has_duplicates.get())
                    });
                    view! {
                        <Show when=move || visible.get()>
                            <TabBtn
                                label=m.tab_title()
                                active=Signal::derive(move || mode.get() == m)
                                on_click=Callback::new(move |_| select_mode(set_mode, m))
                            />
                        </Show>
                    }
                })
                .collect_view()}
        </nav>
    }
}

#[component]
fn TabBtn(label: String, active: Signal<bool>, on_click: Callback<()>) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || {
                if active.get() {
                    "tab-node tab-node--active"
                } else {
                    "tab-node"
                }
            }
            on:click=move |_| on_click.run(())
        >
            {label}
        </button>
    }
}

#[component]
fn FooterNodes(flags: StoredValue<CatalogFlags>, search: CatalogSearch) -> impl IntoView {
    let strip = search.strip_visible;
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let help = HelpOverlay::expect();

    Effect::new(move |_| {
        if search.active.get()
            && let Some(el) = input_ref.get()
        {
            let _ = el.focus();
        }
    });

    view! {
        <div class="footer-nodes">
            <div class="footer-nodes__start">
                <Show
                    when=move || strip.get()
                    fallback=move || {
                        view! {
                            <Show
                                when=move || flags.get_value().last_snapshot_ns.is_some()
                                fallback=|| ().into_any()
                            >
                                <button
                                    type="button"
                                    class="status-node status-node--button"
                                    title="Open catalog search (/)"
                                    on:click=move |_| search.start()
                                >
                                    {
                                        move || {
                                            flags
                                                .get_value()
                                                .last_snapshot_ns
                                                .map(format_timestamp_ns)
                                                .map(|t| format!("Last Snapshot: {t}"))
                                                .unwrap_or_default()
                                        }
                                    }
                                </button>
                            </Show>
                        }
                        .into_any()
                    }
                >
                    <div
                        class=move || {
                            if search.active.get() {
                                "catalog-search catalog-search--active"
                            } else {
                                "catalog-search"
                            }
                        }
                        on:click=move |_| search.start()
                    >
                        <span class="catalog-search__label">{SEARCH_LABEL}</span>
                        <Show
                            when=move || search.active.get()
                            fallback=move || {
                                view! {
                                    <span class="catalog-search__query">
                                        {move || search.query.get()}
                                    </span>
                                }
                                .into_any()
                            }
                        >
                            <input
                                node_ref=input_ref
                                type="text"
                                class="catalog-search__input"
                                prop:value=move || search.query.get()
                                on:input=move |ev| {
                                    search.set_query.set(event_target_value(&ev));
                                }
                                on:blur=move |_| {
                                    // Leave typing mode on blur so digit/mode hotkeys work again;
                                    // keep query (strip stays visible) like TUI after Enter.
                                    search.set_active.set(false);
                                }
                                on:keydown=move |ev| {
                                    let key = ev.key();
                                    if key == "Escape" {
                                        ev.prevent_default();
                                        search.clear();
                                    } else if key == "Enter" {
                                        ev.prevent_default();
                                        search.submit();
                                    }
                                }
                            />
                        </Show>
                    </div>
                </Show>
            </div>
            <button
                type="button"
                class="status-node status-node--button status-node--end"
                title="Keyboard help (?)"
                on:click=move |_| help.toggle()
            >
                "? — Help"
            </button>
        </div>
    }
}
