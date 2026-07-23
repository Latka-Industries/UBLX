use anyhow::Result;
use std::{
    collections::{HashSet, hash_map::Entry},
    env,
    ffi::OsStr,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use log::debug;

/// Stable names for on-disk artifacts under an indexed root (`ubli/` cache, project `ublx.toml`, export dirs).
///
/// Values follow the Cargo package name where noted (`ublx` → e.g. `ublx.toml`, `.ublx`, `ublx-export/`).
pub struct UblxNames {
    /// Crate / CLI name (`CARGO_PKG_NAME`).
    pub pkg_name: &'static str,
    /// User cache directory name for per-root DB files (e.g. `ubli`).
    pub pkg_name_plural: &'static str,
    /// Index `SQLite` file extension (e.g. `.ublx`).
    pub index_db_file_ext: &'static str,
    /// Visible local config basename (e.g. `ublx.toml`).
    pub local_config_visible_toml: &'static str,
    /// Hidden local config basename (e.g. `.ublx.toml`).
    pub local_config_hidden_toml: &'static str,
    /// Nefaxer DB directory name
    pub nefax_db: &'static str,
    /// Subdirectory for Zahir JSON export (e.g. `ublx-export/`; CLI `-x` / Command Mode).
    pub zahir_export_dir_name: &'static str,
    /// Subdirectory for lens Markdown export (e.g. `ublx-lenses/`; Command Mode).
    pub lens_export_dir_name: &'static str,
}

impl Default for UblxNames {
    fn default() -> Self {
        Self::new()
    }
}

impl UblxNames {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pkg_name: env!("CARGO_PKG_NAME"),
            pkg_name_plural: "ubli",
            index_db_file_ext: concat!(".", env!("CARGO_PKG_NAME")),
            local_config_visible_toml: concat!(env!("CARGO_PKG_NAME"), ".toml"),
            local_config_hidden_toml: concat!(".", env!("CARGO_PKG_NAME"), ".toml"),
            nefax_db: ".nefaxer",
            zahir_export_dir_name: concat!(env!("CARGO_PKG_NAME"), "-export"),
            lens_export_dir_name: concat!(env!("CARGO_PKG_NAME"), "-lenses"),
        }
    }
}

pub const UBLX_NAMES: UblxNames = UblxNames::new();

/// `path_to_hex` / DB stem suffix length (16 hex chars from `DefaultHasher`).
const PATH_HASH_HEX_LEN: usize = 16;

/// True if `s` is exactly 16 ASCII hex digits (matches [`path_to_hex`] output).
#[must_use]
pub fn is_hex_hash16(s: &str) -> bool {
    s.len() == PATH_HASH_HEX_LEN && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Last segment of a DB stem after the final `_`, when it is a 16-char path hash (`{safe}_{hash}`).
#[must_use]
pub fn hash_suffix_from_db_stem(stem: &str) -> Option<&str> {
    let (_, rest) = stem.rsplit_once('_')?;
    is_hex_hash16(rest).then_some(rest)
}

/// Stable hex string for a path (for cache filenames). Same path => same string.
#[must_use]
pub fn path_to_hex(path: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn sanitize_name_for_fs(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "root".to_string()
    } else {
        trimmed.to_string()
    }
}

/// User config directory for ublx. Global config lives here (e.g. `ublx.toml`).
/// - **Unix (macOS, Linux):** `~/.config/ublx`
/// - **Windows:** `%APPDATA%\ublx`
///   Returns `None` if the underlying env (e.g. `HOME`, `APPDATA`) is not set.
fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join(UBLX_NAMES.pkg_name))
    }
    #[cfg(not(windows))]
    {
        env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config").join(UBLX_NAMES.pkg_name))
    }
}

/// User cache/data directory for ublx.
/// - **Unix (macOS, Linux):** `~/.local/share/ublx`
/// - **Windows:** `%LOCALAPPDATA%\ublx`
///   Returns `None` if the underlying env (e.g. `HOME`, `LOCALAPPDATA`) is not set.
fn cache_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join(UBLX_NAMES.pkg_name))
    }
    #[cfg(not(windows))]
    {
        env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join(".local")
                .join("share")
                .join(UBLX_NAMES.pkg_name)
        })
    }
}

/// Per-project `SQLite` files live under `cache_dir()/ubli/` (e.g. `~/.local/share/ublx/ubli`).
#[must_use]
fn db_dir() -> Option<PathBuf> {
    cache_dir().map(|c| c.join(UBLX_NAMES.pkg_name_plural))
}

/// Per-indexed-dir metadata for welcome-screen recents: `cache_dir()/recents/<path_hash>.txt`.
const RECENTS_SUBDIR: &str = "recents";

/// Weight for [`times_opened`] in [`recents_composite_score`]: each session open adds this many
/// effective nanoseconds so frequently opened roots stay competitive vs raw `last_open_ns`.
const RECENTS_OPEN_WEIGHT_NS: u128 = 3_600_000_000_000; // 1 hour per open

#[must_use]
fn recents_dir() -> Option<PathBuf> {
    cache_dir().map(|c| c.join(RECENTS_SUBDIR))
}

#[must_use]
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

#[derive(Debug, Clone)]
struct RecentsFileData {
    path: PathBuf,
    times_opened: u64,
    last_open_ns: u64,
}

fn fmt_recents_txt(data: &RecentsFileData) -> String {
    format!(
        "path={}\ntimes_opened={}\nlast_open_ns={}\n",
        data.path.to_string_lossy(),
        data.times_opened,
        data.last_open_ns
    )
}

fn parse_recents_txt(content: &str) -> Option<RecentsFileData> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.contains('=') {
        let p = PathBuf::from(trimmed);
        return Some(RecentsFileData {
            path: p,
            times_opened: 0,
            last_open_ns: 0,
        });
    }
    let mut path: Option<PathBuf> = None;
    let mut times_opened: u64 = 0;
    let mut last_open_ns: u64 = 0;
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        match k.trim() {
            "path" => path = Some(PathBuf::from(v.trim())),
            "times_opened" => times_opened = v.trim().parse().unwrap_or(0),
            "last_open_ns" => last_open_ns = v.trim().parse().unwrap_or(0),
            _ => {}
        }
    }
    path.map(|p| RecentsFileData {
        path: p,
        times_opened,
        last_open_ns,
    })
}

fn read_recents_file(path: &Path) -> Option<RecentsFileData> {
    let s = fs::read_to_string(path).ok()?;
    parse_recents_txt(&s)
}

/// Composite ordering: mostly `last_open_ns`, with a boost from `times_opened`.
#[must_use]
fn recents_composite_score(data: &RecentsFileData) -> u128 {
    u128::from(data.last_open_ns)
        .saturating_add(u128::from(data.times_opened).saturating_mul(RECENTS_OPEN_WEIGHT_NS))
}

/// True if `cache_dir()/recents/{path_hash(dir)}.txt` exists (this root was registered after the welcome flow).
#[must_use]
pub fn has_recents_entry_for_dir(dir: &Path) -> bool {
    let Some(recents) = recents_dir() else {
        return false;
    };
    let key = path_to_hex(dir);
    recents.join(format!("{key}.txt")).exists()
}

/// Whether to show the first-run welcome UI for this indexed root.
///
/// **Product rule:** when not in headless snapshot mode, show if the per-root `SQLite` file under [`UblxPaths::db`]
/// (in `cache_dir()/ubli/`) did **not** exist yet **before** [`crate::engine::db_ops::ensure_ublx_and_db`].
/// Recents and local `ublx.toml` are **not** part of this gate.
///
/// Callers should compute `had_ubli_db_file` with `UblxPaths::new(dir).db().exists()` **before**
/// `ensure_ublx_and_db` (same order as [`crate::main`]).
#[must_use]
pub fn should_show_initial_prompt(
    snapshot_only: bool,
    had_index_db_before_ensure: bool,
    had_any_cached_db_before_this_root: bool,
) -> bool {
    let initial_prompt = !snapshot_only && !had_index_db_before_ensure;
    debug!(
        "initial_prompt={initial_prompt} (had_index_db_before_ensure={had_index_db_before_ensure})"
    );
    debug!("cached ublx roots seen before startup: {had_any_cached_db_before_this_root}");
    initial_prompt
}

/// True when the shared `ubli` directory contains at least one DB file.
#[must_use]
pub fn has_any_cached_ublx_db() -> bool {
    let Some(dir) = db_dir() else {
        return false;
    };
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    rd.flatten().any(|e| {
        e.path()
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(UBLX_NAMES.index_db_file_ext))
    })
}

/// Register this root after first-run **UBLX here**: creates or updates `recents` entry (path, `last_open_ns`; `times_opened` starts at 0 and is incremented by [`record_ublx_session_open`] on each post-prompt session).
///
/// # Errors
///
/// Returns an error if the recents directory cannot be created or the recents file cannot be written.
pub fn remember_indexed_root_path(dir: &Path) -> Result<()> {
    let Some(recents) = recents_dir() else {
        return Ok(());
    };
    fs::create_dir_all(&recents)?;
    let key = path_to_hex(dir);
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let path_file = recents.join(format!("{key}.txt"));
    let mut data = read_recents_file(&path_file).unwrap_or(RecentsFileData {
        path: canon.clone(),
        times_opened: 0,
        last_open_ns: 0,
    });
    data.path = canon;
    data.last_open_ns = now_ns();
    fs::write(path_file, fmt_recents_txt(&data))?;
    Ok(())
}

/// Refresh `last_open_ns` when the user picks a prior root from the welcome list (does not create a file).
/// Session `times_opened` is updated when the new process runs [`record_ublx_session_open`].
///
/// # Errors
///
/// Returns an error if the recents file cannot be written.
pub fn record_prior_root_selected(dir: &Path) -> Result<()> {
    let Some(recents) = recents_dir() else {
        return Ok(());
    };
    let key = path_to_hex(dir);
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let path_file = recents.join(format!("{key}.txt"));
    if !path_file.exists() {
        return Ok(());
    }
    let Some(mut data) = read_recents_file(&path_file) else {
        return Ok(());
    };
    data.path = canon;
    data.last_open_ns = now_ns();
    fs::write(path_file, fmt_recents_txt(&data))?;
    Ok(())
}

/// Each normal TUI session for a root that already has a recents file: increment `times_opened`, refresh `last_open_ns`.
/// Does not create a file (first registration is only via [`remember_indexed_root_path`] after **UBLX here**).
///
/// # Errors
///
/// Returns an error if the recents file cannot be written.
pub fn record_ublx_session_open(dir: &Path) -> Result<()> {
    let Some(recents) = recents_dir() else {
        return Ok(());
    };
    let key = path_to_hex(dir);
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let path_file = recents.join(format!("{key}.txt"));
    if !path_file.exists() {
        return Ok(());
    }
    let Some(mut data) = read_recents_file(&path_file) else {
        return Ok(());
    };
    data.path = canon;
    data.times_opened = data.times_opened.saturating_add(1);
    data.last_open_ns = now_ns();
    fs::write(path_file, fmt_recents_txt(&data))?;
    Ok(())
}

/// Collect all recents entries (deduped by canonical path).
fn collect_recents_entries() -> Vec<RecentsFileData> {
    let Some(dir) = recents_dir() else {
        return Vec::new();
    };
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut best: std::collections::HashMap<PathBuf, RecentsFileData> =
        std::collections::HashMap::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let Some(mut data) = read_recents_file(&p) else {
            continue;
        };
        let canon = data
            .path
            .canonicalize()
            .unwrap_or_else(|_| data.path.clone());
        data.path.clone_from(&canon);
        match best.entry(canon) {
            Entry::Occupied(mut o) => {
                let ex = o.get_mut();
                if data.last_open_ns > ex.last_open_ns
                    || (data.last_open_ns == ex.last_open_ns && data.times_opened > ex.times_opened)
                {
                    *ex = data;
                }
            }
            Entry::Vacant(v) => {
                v.insert(data);
            }
        }
    }
    best.into_values().collect()
}

/// Indexed roots that have **both** a valid `recents/{hash}.txt` and the matching **main** DB under `ubli/`.
///
/// For each recents file, the path in the file must resolve to a directory whose expected DB exists, and the
/// **16-hex hash in the recents filename** must match the **hash suffix** of that DB file’s stem (same rule as
/// [`UblxPaths::db_stem`]). Entries with a missing DB, wrong hash, or non-hex filename are skipped.
#[must_use]
pub fn all_indexed_roots_alphabetical() -> Vec<PathBuf> {
    let Some(recents) = recents_dir() else {
        return Vec::new();
    };
    let Ok(rd) = fs::read_dir(&recents) else {
        return Vec::new();
    };

    let mut out: HashSet<PathBuf> = HashSet::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let Some(fname) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_hex_hash16(fname) {
            continue;
        }
        let Some(data) = read_recents_file(&p) else {
            continue;
        };
        let path = data
            .path
            .canonicalize()
            .unwrap_or_else(|_| data.path.clone());
        if !path.is_dir() {
            continue;
        }
        let db_path = UblxPaths::new(&path).db();
        if !db_path.exists() {
            continue;
        }
        let Some(db_stem) = db_path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(hash_from_db) = hash_suffix_from_db_stem(db_stem) else {
            continue;
        };
        if hash_from_db != fname {
            continue;
        }
        out.insert(path);
    }

    let mut paths: Vec<PathBuf> = out.into_iter().collect();
    paths.sort_by_key(|a| a.display().to_string());
    paths
}

/// Prior indexed roots that still look valid (directory exists and has a DB file), excluding `current`.
#[must_use]
pub fn prior_indexed_roots(current: &Path) -> Vec<PathBuf> {
    prior_indexed_roots_scored(current, usize::MAX)
        .into_iter()
        .map(|(p, _)| p)
        .collect()
}

/// Scoring prior indexed roots based on time last opened and times opened
fn prior_indexed_roots_scored(current: &Path, max: usize) -> Vec<(PathBuf, RecentsFileData)> {
    let current_canon = current
        .canonicalize()
        .unwrap_or_else(|_| current.to_path_buf());
    let mut scored: Vec<(PathBuf, RecentsFileData)> = Vec::new();
    for mut data in collect_recents_entries() {
        let dir = data
            .path
            .canonicalize()
            .unwrap_or_else(|_| data.path.clone());
        if dir == current_canon || !dir.is_dir() {
            continue;
        }
        let db = UblxPaths::new(&dir).db();
        if !db.exists() {
            continue;
        }
        data.path.clone_from(&dir);
        scored.push((dir, data));
    }
    scored.sort_by(|a, b| {
        recents_composite_score(&b.1)
            .cmp(&recents_composite_score(&a.1))
            .then_with(|| a.0.cmp(&b.0))
    });
    scored.truncate(max);
    scored
}

/// Same as [`prior_indexed_roots`], but sorted by [`recents_composite_score`], capped.
#[must_use]
pub fn prior_indexed_roots_recent(current: &Path, max: usize) -> Vec<PathBuf> {
    prior_indexed_roots_scored(current, max)
        .into_iter()
        .map(|(p, _)| p)
        .collect()
}

/// Path to the global config file: `config_dir()/ublx.toml`. `None` if [`config_dir`] is unavailable.
#[must_use]
pub fn global_config_toml() -> Option<PathBuf> {
    config_dir().map(|c| c.join(UBLX_NAMES.local_config_visible_toml))
}

/// Path to the cached "last applied" config for this dir: `cache_dir()/configs/[path_hex].toml`.
/// Per-indexed-dir so global + local overlay is cached by path. Fallback when hot reload gets invalid config.
#[must_use]
pub fn last_applied_config_path(dir: &Path) -> Option<PathBuf> {
    cache_dir().map(|c| c.join("configs").join(format!("{}.toml", path_to_hex(dir))))
}

/// True if `path_str` is a relative snapshot path equal only to [`LOCAL_CONFIG_VISIBLE_TOML`] / [`LOCAL_CONFIG_HIDDEN_TOML`] at the indexed root (normalized).
#[must_use]
pub fn rel_path_is_exact_local_config_toml(path_str: &str) -> bool {
    let trim = path_str.trim();
    if Path::new(trim).is_absolute() {
        return false;
    }
    let norm = trim.replace('\\', "/");
    let norm = norm.trim_start_matches("./");
    norm == UBLX_NAMES.local_config_visible_toml || norm == UBLX_NAMES.local_config_hidden_toml
}

/// Paths for the index DB and related files under an indexed `dir_to_ublx_abs`. Filenames use [`INDEX_DB_FILE_EXT`] and related suffixes (`_tmp`, `-wal`, `-shm`).
#[derive(Clone, Debug)]
pub struct UblxPaths {
    pub dir_to_ublx_abs: PathBuf,
}

impl UblxPaths {
    #[must_use]
    pub fn new(dir_to_ublx: &Path) -> Self {
        Self {
            dir_to_ublx_abs: dir_to_ublx.to_path_buf(),
        }
    }

    /// Filename stem (no extension) for the index DB: sanitized dir name + path hash.
    fn db_stem(&self) -> String {
        let dir_name = self
            .dir_to_ublx_abs
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root");
        let safe_name = sanitize_name_for_fs(dir_name);
        let hash = path_to_hex(&self.dir_to_ublx_abs);
        format!("{safe_name}_{hash}")
    }

    /// Full filename for the index DB under [`Self::db_dir`] (stem + [`INDEX_DB_FILE_EXT`]).
    #[must_use]
    fn db_filename(&self) -> String {
        format!("{}{}", self.db_stem(), UBLX_NAMES.index_db_file_ext)
    }

    #[must_use]
    pub fn db_dir(&self) -> Option<PathBuf> {
        db_dir()
    }

    /// Ensure the cache db folder exists.
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when creating the db directory fails.
    pub fn ensure_db_dir(&self) -> Result<PathBuf> {
        let dir = self
            .db_dir()
            .ok_or_else(|| anyhow::anyhow!("could not resolve user cache directory"))?;
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    #[must_use]
    pub fn log_path(&self) -> PathBuf {
        self.dir_to_ublx_abs
            .join(format!("{}.log", UBLX_NAMES.pkg_name))
    }

    /// Hidden config path: `dir_to_ublx_abs/.ublx.toml`.
    #[must_use]
    pub fn hidden_toml(&self) -> PathBuf {
        self.dir_to_ublx_abs
            .join(UBLX_NAMES.local_config_hidden_toml)
    }

    /// Visible config path: `dir_to_ublx_abs/ublx.toml`.
    #[must_use]
    pub fn visible_toml(&self) -> PathBuf {
        self.dir_to_ublx_abs
            .join(UBLX_NAMES.local_config_visible_toml)
    }

    /// True if `path` (relative to `dir_to_ublx_abs`) is the hidden or visible ublx config file.
    #[must_use]
    pub fn is_config_file(&self, path: &Path) -> bool {
        let Some(name) = path.file_name() else {
            return false;
        };
        name == OsStr::new(UBLX_NAMES.local_config_visible_toml)
            || name == OsStr::new(UBLX_NAMES.local_config_hidden_toml)
    }

    /// Path to the config file to use: checks for `dir_to_ublx_abs/.ublx.toml` then `dir_to_ublx_abs/ublx.toml`; returns the first that exists, or `None`.
    #[must_use]
    pub fn toml_path(&self) -> Option<PathBuf> {
        let hidden = self.hidden_toml();
        let visible = self.visible_toml();
        if hidden.exists() {
            Some(hidden)
        } else if visible.exists() {
            Some(visible)
        } else {
            None
        }
    }

    /// Path used when creating or updating local config: existing hidden or visible file if present, otherwise
    /// [`Self::hidden_toml`] (same default as other local-config writers in this crate).
    #[must_use]
    pub fn local_config_path_for_write(&self) -> PathBuf {
        self.toml_path().unwrap_or_else(|| self.hidden_toml())
    }

    /// Main DB file under `cache_dir()` / [`UBLX_NAMES.pkg_name_plural`] (basename + [`INDEX_DB_FILE_EXT`]). `SQLite` creates it if missing.
    #[must_use]
    pub fn db(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(self.db_filename())
    }

    /// Nefaxer index file (e.g. `dir_to_ublx_abs/.nefaxer`). When present, used as prior snapshot before ublx snapshot.
    #[must_use]
    pub fn nefax_db(&self) -> PathBuf {
        self.dir_to_ublx_abs.join(UBLX_NAMES.nefax_db)
    }

    /// Temp file (write-then-rename to [`Self::db`]). Same stem as DB with `_tmp` before [`INDEX_DB_FILE_EXT`].
    #[must_use]
    pub fn tmp(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}_tmp{}",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// WAL file for [`Self::tmp`] when snapshot build uses `journal_mode=WAL`.
    #[must_use]
    pub fn tmp_wal(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}_tmp{}-wal",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// Shared-memory file for [`Self::tmp`] in WAL mode.
    #[must_use]
    pub fn tmp_shm(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}_tmp{}-shm",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// `SQLite` WAL file for [`Self::db`] when WAL mode is on.
    #[must_use]
    pub fn wal(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}{}-wal",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// `SQLite` shared-memory file for [`Self::db`] in WAL mode.
    #[must_use]
    pub fn shm(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}{}-shm",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// Paths to exclude from indexing (nefax + local config). DB files under `ubli/` use [`INDEX_DB_FILE_EXT`] and are not listed here (nefax matches path components).
    /// Local `ublx.toml` / `.ublx.toml` are edited from the Settings tab, not listed as a snapshot category.
    /// [`UblxNames::zahir_export_dir_name`] (flat Zahir JSON export) and [`UblxNames::lens_export_dir_name`] (lens Markdown export) are excluded so re-indexing does not ingest them.
    #[must_use]
    pub fn exclude(&self) -> Vec<String> {
        vec![
            UBLX_NAMES.nefax_db.to_string(),
            UBLX_NAMES.local_config_visible_toml.to_string(),
            UBLX_NAMES.local_config_hidden_toml.to_string(),
            UBLX_NAMES.zahir_export_dir_name.to_string(),
            // UBLX_NAMES.lens_export_dir_name.to_string(),
        ]
    }

    /// Remove tmp, WAL, and SHM files if they exist. No error if any are missing.
    /// Close the DB connection before calling if you use WAL mode.
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when removing an existing auxiliary file fails (e.g. I/O permission denied).
    pub fn remove_aux_files(&self) -> Result<(), anyhow::Error> {
        for p in [
            self.tmp(),
            self.tmp_wal(),
            self.tmp_shm(),
            self.wal(),
            self.shm(),
        ] {
            if p.exists() {
                fs::remove_file(&p)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn global_config(&self) -> Option<PathBuf> {
        global_config_toml()
    }

    /// User cache dir (`~/.local/share/ublx` or Windows equivalent). Used for last-applied config and future hot-reload fallback.
    #[allow(dead_code)]
    #[must_use]
    pub fn cache_dir(&self) -> Option<PathBuf> {
        cache_dir()
    }

    #[must_use]
    pub fn last_applied_config_path(&self) -> Option<PathBuf> {
        last_applied_config_path(&self.dir_to_ublx_abs)
    }
}

#[must_use]
pub fn get_log_path(dir_to_ublx: &Path) -> PathBuf {
    UblxPaths::new(dir_to_ublx).log_path()
}

#[must_use]
/// Normalize a path string for policy matching (e.g. `photos/vacation` → `photos/vacation`)
pub fn normalize_rel_path_for_policy(s: &str) -> String {
    let s = s.replace('\\', "/");
    let s = s.trim_start_matches("./");
    s.trim_end_matches('/').to_string()
}

/// True if `rel` (relative path) is under or equal to `prefix` (e.g. `photos/vacation` is under `photos`).
#[must_use]
pub fn path_is_under_or_equal(rel: &str, prefix: &str) -> bool {
    rel == prefix || (rel.starts_with(prefix) && rel.as_bytes().get(prefix.len()) == Some(&b'/'))
}

#[cfg(test)]
mod tests {
    use super::{UBLX_NAMES, hash_suffix_from_db_stem, is_hex_hash16, should_show_initial_prompt};

    #[test]
    fn never_in_snapshot_only_mode() {
        assert!(!should_show_initial_prompt(true, false, false));
        assert!(!should_show_initial_prompt(true, true, false));
    }

    #[test]
    fn initial_prompt_only_if_no_ubli_db_when_not_snapshot_only() {
        assert!(should_show_initial_prompt(false, false, false));
        assert!(!should_show_initial_prompt(false, true, false));
    }

    #[test]
    fn index_db_file_ext_is_dot_pkg_name() {
        assert_eq!(
            UBLX_NAMES.index_db_file_ext,
            format!(".{}", UBLX_NAMES.pkg_name)
        );
    }

    #[test]
    fn hex_hash16_accepts_16_hex_digits() {
        assert!(is_hex_hash16("729a9c26db109730"));
        assert!(!is_hex_hash16("729a9c26db10973"));
        assert!(!is_hex_hash16("729a9c26db1097300"));
        assert!(!is_hex_hash16("g29a9c26db109730"));
    }

    #[test]
    fn hash_suffix_from_db_stem_parses_stem() {
        assert_eq!(
            hash_suffix_from_db_stem("mydir_729a9c26db109730"),
            Some("729a9c26db109730")
        );
        assert_eq!(hash_suffix_from_db_stem("729a9c26db109730"), None);
        assert_eq!(hash_suffix_from_db_stem("no_hash_here"), None);
    }
}
