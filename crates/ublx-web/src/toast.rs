//! Shared toast stack — shadcn/Sonner-looking cards on a wasm-safe host.
//!
//! `leptos-shadcn-toast` Sonner calls `Instant::now()`, which panics on
//! `wasm32-unknown-unknown`. Same layout/chrome, without that dependency.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::util::sleep_ms;

const MAX_TOASTS: usize = 3;
const DURATION: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToastLevel {
    Info,
    Warn,
    Error,
    Success,
    Loading,
}

impl ToastLevel {
    fn class(self) -> &'static str {
        match self {
            Self::Info => "toast-item--info",
            Self::Warn => "toast-item--warn",
            Self::Error => "toast-item--error",
            Self::Success => "toast-item--success",
            Self::Loading => "toast-item--loading",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Warn => "!",
            Self::Error => "×",
            Self::Success => "✓",
            Self::Loading => "…",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ToastBody {
    Text(String),
    Snapshot {
        added: usize,
        modified: usize,
        removed: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToastItem {
    pub id: u64,
    pub level: ToastLevel,
    pub body: ToastBody,
}

#[derive(Clone, Copy)]
pub(crate) struct ToastCtx {
    pub items: RwSignal<Vec<ToastItem>>,
    next_id: RwSignal<u64>,
}

impl ToastCtx {
    pub(crate) fn provide() -> Self {
        let ctx = Self {
            items: RwSignal::new(Vec::new()),
            next_id: RwSignal::new(1),
        };
        provide_context(ctx);
        ctx
    }

    fn push(self, level: ToastLevel, body: ToastBody) {
        let id = self.next_id.get_untracked();
        self.next_id.set(id.wrapping_add(1));
        let item = ToastItem { id, level, body };
        self.items.update(|v| {
            v.push(item);
            while v.len() > MAX_TOASTS {
                v.remove(0);
            }
        });
        let items = self.items;
        let ms = DURATION.as_millis() as i32;
        spawn_local(async move {
            sleep_ms(ms).await;
            items.update(|v| v.retain(|t| t.id != id));
        });
    }

    pub(crate) fn info(self, msg: impl Into<String>) {
        self.push(ToastLevel::Info, ToastBody::Text(msg.into()));
    }

    pub(crate) fn warn(self, msg: impl Into<String>) {
        self.push(ToastLevel::Warn, ToastBody::Text(msg.into()));
    }

    pub(crate) fn error(self, msg: impl Into<String>) {
        self.push(ToastLevel::Error, ToastBody::Text(msg.into()));
    }

    pub(crate) fn loading(self, msg: impl Into<String>) {
        self.push(ToastLevel::Loading, ToastBody::Text(msg.into()));
    }

    pub(crate) fn snapshot_done(self, added: usize, modified: usize, removed: usize) {
        self.push(
            ToastLevel::Success,
            ToastBody::Snapshot {
                added,
                modified,
                removed,
            },
        );
    }
}

#[component]
pub(crate) fn ToastHost() -> impl IntoView {
    let toasts = expect_context::<ToastCtx>();
    view! {
        <div class="toast-stack" aria-live="polite" aria-relevant="additions">
            <For
                each=move || toasts.items.get()
                key=|t| t.id
                let:t
            >
                {
                    let level = t.level;
                    let level_class = level.class();
                    let icon = level.icon();
                    let body = t.body.clone();
                    let id = t.id;
                    let items = toasts.items;
                    view! {
                        <div class=format!("toast-item {level_class}") role="alert">
                            <div class="toast-item__icon" aria-hidden="true">{icon}</div>
                            <div class="toast-item__body">
                                <div class="toast-item__title">{toast_title_view(body.clone())}</div>
                                {toast_desc_view(body)}
                            </div>
                            <button
                                type="button"
                                class="toast-item__close"
                                aria-label="Dismiss"
                                on:click=move |_| {
                                    items.update(|v| v.retain(|t| t.id != id));
                                }
                            >
                                "×"
                            </button>
                        </div>
                    }
                }
            </For>
        </div>
    }
}

fn toast_title_view(body: ToastBody) -> AnyView {
    match body {
        ToastBody::Text(s) => view! { {s} }.into_any(),
        ToastBody::Snapshot { .. } => view! { "Snapshot done" }.into_any(),
    }
}

fn toast_desc_view(body: ToastBody) -> AnyView {
    match body {
        ToastBody::Text(_) => {
            view! { <div class="toast-item__desc toast-item__desc--empty"></div> }.into_any()
        }
        ToastBody::Snapshot {
            added,
            modified,
            removed,
        } => view! {
            <div class="toast-item__desc">
                <span class="toast-delta toast-delta--added">{format!("+{added}")}</span>
                " "
                <span class="toast-delta toast-delta--mod">{format!("~{modified}")}</span>
                " "
                <span class="toast-delta toast-delta--removed">{format!("-{removed}")}</span>
            </div>
        }
        .into_any(),
    }
}
