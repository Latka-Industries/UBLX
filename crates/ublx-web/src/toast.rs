//! Shared toast stack — shadcn `Toast` chrome, TUI bottom-right stacking.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_shadcn_ui::Toast;

use crate::util::sleep_ms;

/// Match TUI toast config (`src/config/toast.rs`): max 3, ~4s.
const MAX_TOASTS: usize = 3;
const DURATION: Duration = Duration::from_secs(4);

/// Severity → shadcn `Toast` variant + CSS level class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToastLevel {
    Info,
    Warn,
    Error,
}

impl ToastLevel {
    fn variant(self) -> &'static str {
        match self {
            Self::Info => "default",
            Self::Warn => "warning",
            Self::Error => "destructive",
        }
    }

    fn class(self) -> &'static str {
        match self {
            Self::Info => "toast-item--info",
            Self::Warn => "toast-item--warn",
            Self::Error => "toast-item--error",
        }
    }
}

/// Toast payload — plain text or a Snapshot delta summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ToastBody {
    Text(String),
    /// Snapshot summary — `+added ~mod -removed` use delta token colors.
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

/// App-wide toast stack (max [`MAX_TOASTS`], auto-dismiss after [`DURATION`]).
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

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
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

    pub(crate) fn snapshot_done(self, added: usize, modified: usize, removed: usize) {
        self.push(
            ToastLevel::Info,
            ToastBody::Snapshot {
                added,
                modified,
                removed,
            },
        );
    }
}

/// Bottom-right stack: oldest above, newest nearest the footer (TUI `slots.iter().rev()`).
#[component]
pub(crate) fn ToastHost() -> impl IntoView {
    let toasts = ToastCtx::expect();
    view! {
        <div class="toast-stack" aria-live="polite" aria-relevant="additions">
            <For
                each=move || toasts.items.get()
                key=|t| t.id
                let:t
            >
                {
                    let variant = t.level.variant().to_string();
                    let level_class = t.level.class();
                    let body = t.body.clone();
                    view! {
                        <Toast
                            variant=variant
                            class=format!("toast-item {level_class} w-full max-w-sm pointer-events-auto")
                        >
                            <div class="toast-item__msg text-sm whitespace-pre-wrap break-words">
                                {toast_body_view(body)}
                            </div>
                        </Toast>
                    }
                }
            </For>
        </div>
    }
}

fn toast_body_view(body: ToastBody) -> AnyView {
    match body {
        ToastBody::Text(s) => view! { {s} }.into_any(),
        ToastBody::Snapshot {
            added,
            modified,
            removed,
        } => view! {
            "Snapshot done "
            <span class="toast-delta toast-delta--added">{format!("+{added}")}</span>
            " "
            <span class="toast-delta toast-delta--mod">{format!("~{modified}")}</span>
            " "
            <span class="toast-delta toast-delta--removed">{format!("-{removed}")}</span>
        }
        .into_any(),
    }
}
