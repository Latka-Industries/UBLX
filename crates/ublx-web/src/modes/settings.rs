//! Settings mode stub.

use leptos::prelude::*;

use crate::panes::ThreePane;

#[component]
pub(crate) fn SettingsMode() -> impl IntoView {
    view! {
        <ThreePane
            left_title="Scope"
            middle_title="Options"
            left=view! {
                <ul class="panel-list">
                    <li><button type="button" class="panel-row panel-row--selected">
                        <span class="panel-row__sym">"›"</span>
                        <span class="panel-row__text">"Global"</span>
                    </button></li>
                    <li><button type="button" class="panel-row">
                        <span class="panel-row__sym">" "</span>
                        <span class="panel-row__text">"Local"</span>
                    </button></li>
                </ul>
            }
            .into_any()
            middle=view! { <p class="pane-empty">"Settings rows"</p> }.into_any()
            right=view! {
                <div class="right-pane">
                    <div class="panel-titlebar">
                        <span class="tab-node tab-node--sm">"Description"</span>
                    </div>
                    <div class="panel-pad pane-empty">"Option help / preview"</div>
                </div>
            }
            .into_any()
        />
    }
}
