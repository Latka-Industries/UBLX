//! Bordered 3-pane layout (ratatui `Block`-style) and right-pane tabs.

mod panel;
mod status;

use leptos::prelude::*;

use crate::focus::{PaneFocus, UiNav};

use self::panel::PanelBox;

pub(crate) use self::panel::{EntryRightPane, OverviewRightPane, PanelRow, PathsPane};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RightTab {
    Viewer,
    Templates,
    Metadata,
    Writing,
}

impl RightTab {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Viewer => "Viewer",
            Self::Templates => "Templates",
            Self::Metadata => "Metadata",
            Self::Writing => "Writing",
        }
    }
}

/// Shared 3-pane TUI layout — bordered boxes with title nodes.
/// Pane focus lives in [`UiNav`] (keyboard + click).
#[component]
pub(crate) fn ThreePane(
    left_title: &'static str,
    middle_title: &'static str,
    left: AnyView,
    middle: AnyView,
    right: AnyView,
) -> impl IntoView {
    let nav = UiNav::expect();
    let focus = nav.pane;
    let set_focus = nav.set_pane;

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
                focused=Signal::derive(|| false)
                on_focus=Callback::new(|_| {})
            >
                {right}
            </PanelBox>
        </div>
    }
}
