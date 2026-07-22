//! Lenses mode stub.

use leptos::prelude::*;

use crate::panes::{RightPaneShell, ThreePane};

#[component]
pub(crate) fn LensesMode() -> impl IntoView {
    view! {
        <ThreePane
            left_title="Lenses"
            middle_title="Paths"
            left=view! { <p class="pane-empty">"Lens names"</p> }.into_any()
            middle=view! { <p class="pane-empty">"Lens member paths"</p> }.into_any()
            right=view! { <RightPaneShell/> }.into_any()
        />
    }
}
