//! HTML render of host-parsed Metadata / Writing [`SectionView`]s (TUI kv_tables).

use leptos::prelude::*;

use crate::api::SectionView;

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
    }
}
