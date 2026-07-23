//! `ublx serve` — local HTTP API over `.ublx` (THI-156 / v0.1.13).

mod catalog;
mod content;
mod error;
mod roots;
mod settings;
mod snapshot;
mod state;

#[cfg(feature = "ui")]
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::get;
use log::info;
#[cfg(feature = "ui")]
use log::warn;
use panza::{ServeMeta, StaticMount, run as panza_run};

use crate::app::tokio_rt;
use crate::cli::catalog::open_catalog_for_read;
use crate::cli_parser::ServeCli;

use self::catalog::{
    get_categories, get_delta, get_duplicates, get_entries, get_entry, get_lens, get_lenses,
};
use self::content::get_entry_content;
use self::roots::{get_current_root, get_doctor, get_roots, put_current_root};
use self::settings::{get_settings, patch_settings_route};
use self::snapshot::{get_snapshot, post_snapshot};
use self::state::{AppState, AppStateInner, ServeCatalog, SnapshotJob};

/// Run `ublx serve` until the process is interrupted.
///
/// # Errors
///
/// Returns `Err` when the catalog cannot be opened or the server fails to bind.
pub fn run(args: &ServeCli) -> Result<(), anyhow::Error> {
    let handle = open_catalog_for_read(&args.dir)?;
    info!(
        "serve catalog ready: dir={} db={}",
        handle.paths.dir.display(),
        handle.read_path.display()
    );
    let state: AppState = Arc::new(Mutex::new(AppStateInner {
        catalog: ServeCatalog {
            dir: handle.paths.dir,
            read_path: handle.read_path,
            conn: handle.conn,
        },
        snapshot: SnapshotJob::idle(),
    }));

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
        .route("/lenses", get(get_lenses))
        .route("/lenses/{name}", get(get_lens))
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

/// Optional Leptos UI (`--features ui`): serve `crates/ublx-web/dist` (or `UBLX_WEB_DIST`).
fn static_mount() -> StaticMount {
    #[cfg(feature = "ui")]
    {
        let dir = std::env::var_os("UBLX_WEB_DIST")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/ublx-web/dist"));
        if !dir.join("index.html").is_file() {
            warn!(
                "feature `ui` enabled but {}/index.html missing — run crates/ublx-web/build.sh",
                dir.display()
            );
        } else {
            info!("serve UI static mount: {}", dir.display());
        }
        StaticMount::Dir(dir)
    }
    #[cfg(not(feature = "ui"))]
    {
        StaticMount::None
    }
}
