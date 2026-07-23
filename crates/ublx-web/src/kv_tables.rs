//! HTML render of host-parsed Metadata / Writing [`SectionView`]s (TUI kv_tables).

use leptos::html::Div;
use leptos::prelude::*;

use crate::api::{SectionView, TreeNodeView};
use crate::focus::install_tree_collapse_on;

#[component]
pub(crate) fn KvTables(sections: Vec<SectionView>) -> impl IntoView {
    view! {
        <div class="kv-tables">
            {sections
                .into_iter()
                .map(|section| {
                    view! { <KvSection section=section/> }
                })
                .collect_view()}
        </div>
    }
}

#[component]
fn KvSection(section: SectionView) -> impl IntoView {
    match section {
        SectionView::KeyValue {
            title,
            sub_title,
            rows,
        } => {
            let title_class = if sub_title {
                "kv-section__title kv-section__title--sub"
            } else {
                "kv-section__title"
            };
            view! {
                <section class="kv-section">
                    {title.map(|t| {
                        view! { <h3 class=title_class>{t}</h3> }
                    })}
                    <div class="kv-table-scroll">
                        <table class="kv-table">
                            <thead>
                                <tr>
                                    <th>"Key"</th>
                                    <th>"Value"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {rows
                                    .into_iter()
                                    .map(|r| {
                                        view! {
                                            <tr>
                                                <td class="kv-table__key">{r.key}</td>
                                                <td class="kv-table__value">{r.value}</td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()}
                            </tbody>
                        </table>
                    </div>
                </section>
            }
            .into_any()
        }
        SectionView::Contents {
            title,
            sub_title,
            columns,
            rows,
        } => {
            let title_class = if sub_title {
                "kv-section__title kv-section__title--sub"
            } else {
                "kv-section__title"
            };
            view! {
                <section class="kv-section">
                    <h3 class=title_class>{title}</h3>
                    <div class="kv-table-scroll">
                        <table class="kv-table kv-table--wide">
                            <thead>
                                <tr>
                                    {columns
                                        .into_iter()
                                        .map(|c| view! { <th>{c}</th> })
                                        .collect_view()}
                                </tr>
                            </thead>
                            <tbody>
                                {rows
                                    .into_iter()
                                    .map(|row| {
                                        view! {
                                            <tr>
                                                {row
                                                    .into_iter()
                                                    .map(|cell| {
                                                        view! { <td class="kv-table__value">{cell}</td> }
                                                    })
                                                    .collect_view()}
                                            </tr>
                                        }
                                    })
                                    .collect_view()}
                            </tbody>
                        </table>
                    </div>
                </section>
            }
            .into_any()
        }
        SectionView::SingleColumnList { title, values } => view! {
            <section class="kv-section">
                <h3 class="kv-section__title">{title}</h3>
                <ul class="kv-list">
                    {values
                        .into_iter()
                        .map(|v| view! { <li>{v}</li> })
                        .collect_view()}
                </ul>
            </section>
        }
        .into_any(),
        SectionView::Tree { title, roots } => view! {
            <section class="kv-section">
                <h3 class="kv-section__title">{title}</h3>
                <CollapsibleTree roots=roots class="kv-table-scroll"/>
            </section>
        }
        .into_any(),
    }
}

/// Shared collapsible tree host (directory Viewer + Metadata schema). Registers expand/collapse
/// controls only when at least one expandable node exists.
#[component]
pub(crate) fn CollapsibleTree(
    roots: Vec<TreeNodeView>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let has_collapsible = tree_has_collapsible(&roots);
    let root_ref = NodeRef::<Div>::new();

    Effect::new(move |_| {
        if !has_collapsible {
            return;
        }
        let Some(root) = root_ref.get() else {
            return;
        };
        install_tree_collapse_on(&root);
    });

    let class = if class.is_empty() {
        "schema-tree text-viewer".to_string()
    } else {
        format!("schema-tree {class}")
    };
    view! {
        <div class=class node_ref=root_ref>
            {roots
                .into_iter()
                .map(|node| {
                    view! { <CollapsibleTreeNode node=node depth=0/> }
                })
                .collect_view()}
        </div>
    }
}

fn tree_has_collapsible(nodes: &[TreeNodeView]) -> bool {
    nodes
        .iter()
        .any(|n| n.branch || !n.children.is_empty() || tree_has_collapsible(&n.children))
}

#[component]
pub(crate) fn CollapsibleTreeNode(node: TreeNodeView, depth: u16) -> impl IntoView {
    let TreeNodeView {
        label,
        value,
        children,
        branch,
    } = node;
    let expandable = branch || !children.is_empty();
    let open = depth < 1;

    if expandable {
        view! {
            <details class="schema-tree__node" prop:open=open>
                <summary class="schema-tree__summary">
                    <span class="schema-tree__label">{label.clone()}</span>
                    {value.clone().map(|v| {
                        view! {
                            <span class="schema-tree__value">{format!(": {v}")}</span>
                        }
                    })}
                </summary>
                <div class="schema-tree__children">
                    {children
                        .into_iter()
                        .map(|child| {
                            view! { <CollapsibleTreeNode node=child depth=depth.saturating_add(1)/> }
                        })
                        .collect_view()}
                </div>
            </details>
        }
        .into_any()
    } else {
        view! {
            <div class="schema-tree__leaf">
                <span class="schema-tree__label">{label}</span>
                {value.map(|v| {
                    view! {
                        <span class="schema-tree__value">{format!(": {v}")}</span>
                    }
                })}
            </div>
        }
        .into_any()
    }
}
