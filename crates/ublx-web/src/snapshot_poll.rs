//! Poll `GET /snapshot` until idle — Command Mode `s` and cold-start auto-index.

use crate::api::{SnapshotLast, get_snapshot_status};
use crate::catalog_refresh::{CatalogRefresh, CatalogScope};
use crate::toast::ToastCtx;
use crate::util::sleep_ms;

/// If a snapshot is already `running` (serve cold start), toast + poll until done and refresh.
pub(crate) async fn watch_snapshot_if_running(refresh: CatalogRefresh, toasts: ToastCtx) {
    match get_snapshot_status().await {
        Ok(st) if st.state.eq_ignore_ascii_case("running") => {
            toasts.loading("Indexing catalog…");
            poll_until_settled(refresh, toasts).await;
        }
        _ => {}
    }
}

/// Poll after `POST /snapshot` until `done` / `failed` / non-running.
pub(crate) async fn poll_until_settled(refresh: CatalogRefresh, toasts: ToastCtx) {
    for _ in 0..600 {
        sleep_ms(500).await;
        match get_snapshot_status().await {
            Ok(st) if st.state.eq_ignore_ascii_case("running") => continue,
            Ok(st) if st.state.eq_ignore_ascii_case("done") => {
                refresh.bump(CatalogScope::ALL);
                match st.last {
                    Some(SnapshotLast {
                        added,
                        modified,
                        removed,
                        ..
                    }) => toasts.snapshot_done(added, modified, removed),
                    None => toasts.info("Snapshot done"),
                }
                return;
            }
            Ok(st) if st.state.eq_ignore_ascii_case("failed") => {
                let msg = st
                    .last
                    .and_then(|l| l.error)
                    .unwrap_or_else(|| "Snapshot failed".into());
                toasts.error(msg);
                return;
            }
            Ok(_) => {
                refresh.bump(CatalogScope::ALL);
                toasts.info("Snapshot finished");
                return;
            }
            Err(e) => {
                toasts.error(e);
                return;
            }
        }
    }
    toasts.warn("Snapshot still running — check later");
}
