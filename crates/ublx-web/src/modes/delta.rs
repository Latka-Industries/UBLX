//! Delta mode stub.

use leptos::prelude::*;

use crate::panes::{RightPaneShell, ThreePane};

#[component]
pub(crate) fn DeltaMode() -> impl IntoView {
    view! {
        <ThreePane
            left_title="Change"
            middle_title="Paths"
            left=view! { <p class="pane-empty">"added / mod / removed"</p> }.into_any()
            middle=view! { <p class="pane-empty">"Changed paths"</p> }.into_any()
            right=view! { <RightPaneShell overview_only=true/> }.into_any()
        />
    }
}
