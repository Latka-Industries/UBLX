//! Duplicates mode stub (needs serve API later).

use leptos::prelude::*;

use crate::panes::{RightPaneShell, ThreePane};

#[component]
pub(crate) fn DuplicatesMode() -> impl IntoView {
    view! {
        <ThreePane
            left_title="Groups"
            middle_title="Members"
            left=view! { <p class="pane-empty">"Duplicate groups"</p> }.into_any()
            middle=view! { <p class="pane-empty">"Group members"</p> }.into_any()
            right=view! { <RightPaneShell/> }.into_any()
        />
    }
}
