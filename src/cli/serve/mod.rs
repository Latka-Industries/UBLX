//! `ublx serve` — local HTTP API over `.ublx` (THI-156 / v0.1.13).

mod catalog;
mod content;
mod error;
mod export;
mod fs;
mod lenses_mut;
mod roots;
mod settings;
mod snapshot;
mod state;
#[cfg(feature = "ui")]
mod web_embed;

#[cfg(feature = "ui")]
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{get, post};
use log::info;
#[cfg(feature = "ui")]
use log::warn;
use panza::{ServeMeta, StaticMount, run as panza_run};

use crate::app::tokio_rt;
use crate::cli::catalog::{
    catalog_db_present, open_catalog_for_read, open_placeholder_catalog, resolve_catalog_paths,
};
use crate::cli_parser::ServeCli;
use crate::config::{record_ublx_session_open, remember_indexed_root_path};

use self::catalog::{
    get_categories, get_delta, get_duplicates, get_entries, get_entry, get_lens, get_lenses,
};
use self::content::get_entry_content;
use self::export::{post_export_lenses, post_export_zahir};
use self::fs::{post_bulk_rename, post_delete, post_enhance, post_enhance_policy, post_rename};
use self::lenses_mut::{
    delete_lens_paths, delete_lens_route, patch_lens, post_lens, post_lens_paths,
};
use self::roots::{get_current_root, get_doctor, get_roots, put_current_root};
use self::settings::{get_settings, patch_settings_route};
use self::snapshot::{begin_snapshot_job, get_snapshot, post_snapshot};
use self::state::{AppState, AppStateInner, ServeCatalog, SnapshotJob};

/// Run `ublx serve` until the process is interrupted.
///
/// When no catalog exists, starts with an in-memory placeholder and kicks a background snapshot
/// (TUI first-run parity) unless `--no-auto-snapshot` is set.
///
/// # Errors
///
/// Returns `Err` when the catalog cannot be opened (or is missing with `--no-auto-snapshot`)
/// or the server fails to bind.
pub fn run(args: &ServeCli) -> Result<(), anyhow::Error> {
    let catalog_paths = resolve_catalog_paths(&args.dir)?;
    let missing = !catalog_db_present(&catalog_paths);
    if missing && args.no_auto_snapshot {
        anyhow::bail!(
            "no catalog DB for {} (expected {}); run `ublx` or `ublx -s` in that directory first \
             (or omit --no-auto-snapshot to index on serve start)",
            catalog_paths.dir.display(),
            catalog_paths.db_path.display()
        );
    }

    let handle = if missing {
        info!(
            "serve cold start: no catalog for {} — placeholder + auto-snapshot",
            catalog_paths.dir.display()
        );
        open_placeholder_catalog(catalog_paths)?
    } else {
        open_catalog_for_read(&args.dir)?
    };

    info!(
        "serve catalog ready: dir={} db={}",
        handle.paths.dir.display(),
        handle.read_path.display()
    );
    // Same as TUI session open: ensure `recents/{hash}.txt` exists so the switcher lists this root.
    let _ = remember_indexed_root_path(&handle.paths.dir);
    let _ = record_ublx_session_open(&handle.paths.dir);

    let state: AppState = Arc::new(Mutex::new(AppStateInner {
        catalog: ServeCatalog {
            dir: handle.paths.dir,
            read_path: handle.read_path,
            conn: handle.conn,
        },
        snapshot: SnapshotJob::idle(),
    }));

    if missing {
        begin_snapshot_job(&state, false).map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    let api = Router::new()
        .route("/roots", get(get_roots))
        .route(
            "/roots/current",
            get(get_current_root).put(put_current_root),
        )
        .route("/doctor", get(get_doctor))
        .route("/snapshot", get(get_snapshot).post(post_snapshot))
        .route("/categories", get(get_categories))
        .route("/entries", get(get_entries))
        .route("/entries/{*path}", get(get_entry))
        .route("/content/{*path}", get(get_entry_content))
        .route("/delta", get(get_delta))
        .route("/duplicates", get(get_duplicates))
        .route("/lenses", get(get_lenses).post(post_lens))
        .route(
            "/lenses/{name}",
            get(get_lens).patch(patch_lens).delete(delete_lens_route),
        )
        .route(
            "/lenses/{name}/paths",
            post(post_lens_paths).delete(delete_lens_paths),
        )
        .route("/fs/rename", post(post_rename))
        .route("/fs/bulk-rename", post(post_bulk_rename))
        .route("/fs/delete", post(post_delete))
        .route("/fs/enhance", post(post_enhance))
        .route("/fs/enhance-policy", post(post_enhance_policy))
        .route("/export/zahir", post(post_export_zahir))
        .route("/export/lenses", post(post_export_lenses))
        .route(
            "/settings/{scope}",
            get(get_settings).patch(patch_settings_route),
        )
        .with_state(state);

    tokio_rt::runtime().block_on(panza_run(
        ServeMeta {
            service: "ublx",
            version: env!("CARGO_PKG_VERSION"),
        },
        args.serve.clone(),
        api,
        static_mount(),
    ))
}

/// Optional Leptos UI (`--features ui`): Embedded by default; `UBLX_WEB_DIST` → Dir (dev loop).
fn static_mount() -> StaticMount {
    #[cfg(feature = "ui")]
    {
        if let Some(dir) = std::env::var_os("UBLX_WEB_DIST").map(PathBuf::from) {
            if !dir.join("index.html").is_file() {
                warn!(
                    "UBLX_WEB_DIST={}/index.html missing — run crates/ublx-web/build.sh",
                    dir.display()
                );
            } else {
                info!("serve UI static mount (Dir): {}", dir.display());
            }
            return StaticMount::Dir(dir);
        }
        let assets = web_embed::embedded_assets();
        if assets.contains_key("index.html") {
            info!("serve UI static mount (Embedded): {} assets", assets.len());
        } else {
            warn!(
                "feature `ui` Embedded map has no index.html — rebuild after crates/ublx-web/build.sh"
            );
        }
        StaticMount::Embedded(assets)
    }
    #[cfg(not(feature = "ui"))]
    {
        StaticMount::None
    }
}
