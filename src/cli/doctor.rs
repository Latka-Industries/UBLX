//! `ublx doctor` — diagnose `.ublx` DB / path / schema (THI-154).

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::cli::catalog::{
    CatalogHandle, CatalogPaths, open_catalog_for_read, resolve_catalog_paths,
    snapshot_likely_in_progress,
};
use crate::cli::output::emit_json;
use crate::cli_parser::DoctorCli;
use crate::engine::db_ops::SnapshotReaderPreference;
use crate::engine::db_ops::snapshot_reader_path_with;
use crate::utils::{EXIT_ERROR, format_bytes};

const EXPECTED_TABLES: &[&str] = &[
    "snapshot",
    "settings",
    "delta_log",
    "path",
    "lens",
    "lens_path",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Warn => "WARN",
            Self::Fail => "FAIL",
        }
    }

    fn worse(self, other: Self) -> Self {
        use Status::{Fail, Pass, Warn};
        match (self, other) {
            (Fail, _) | (_, Fail) => Fail,
            (Warn, _) | (_, Warn) => Warn,
            (Pass, Pass) => Pass,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct ArtifactInfo {
    pub kind: String,
    pub path: String,
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CatalogStats {
    pub snapshot_rows: i64,
    pub distinct_categories: i64,
    pub delta_rows: i64,
    pub lenses: i64,
    pub lens_paths: i64,
    pub path_rows: i64,
    pub null_hashes: i64,
    pub empty_categories: i64,
    pub settings_row: bool,
}

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub dir: String,
    pub db_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_path: Option<String>,
    pub artifacts: Vec<ArtifactInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<CatalogStats>,
    /// Aux paths removed by `--fix` (kinds: tmp, wal, shm, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<Vec<String>>,
    pub checks: Vec<Check>,
    pub summary: Status,
}

impl DoctorReport {
    fn new(catalog_paths: &CatalogPaths) -> Self {
        Self {
            dir: catalog_paths.dir.display().to_string(),
            db_path: catalog_paths.db_path.display().to_string(),
            read_path: None,
            artifacts: collect_artifacts(catalog_paths),
            journal_mode: None,
            stats: None,
            removed: None,
            checks: Vec::new(),
            summary: Status::Pass,
        }
    }

    fn push(&mut self, name: impl Into<String>, status: Status, detail: impl Into<String>) {
        self.summary = self.summary.worse(status);
        self.checks.push(Check {
            name: name.into(),
            status,
            detail: detail.into(),
        });
    }
}

/// Diagnose a catalog directory (no `--fix`, no process exit).
///
/// When a snapshot appears in progress, returns a report with `summary: fail` and a
/// `snapshot_lock` check (same as CLI without `--force`) instead of exiting.
///
/// # Errors
///
/// Returns `Err` when the directory is invalid / cannot be resolved.
pub fn diagnose(dir: &Path) -> Result<DoctorReport, anyhow::Error> {
    let catalog_paths = resolve_catalog_paths(dir)?;
    if snapshot_likely_in_progress(&catalog_paths) {
        return Ok(report_blocked_by_snapshot(&catalog_paths));
    }
    Ok(diagnose_with_paths(&catalog_paths))
}

/// Run `ublx doctor`. Prints a PASS/WARN/FAIL report; exits non-zero on FAIL.
///
/// With `--fix`, removes leftover tmp/wal/shm aux files after the diagnosis pass
/// (connection closed first). Does not delete the main `.ublx` DB.
///
/// Blocked when a snapshot appears in progress (`.ublx_tmp` + tmp wal/shm) unless `--force`.
///
/// # Errors
///
/// Returns `Err` when the directory is invalid, aux cleanup fails, or JSON serialization fails.
pub fn run(args: &DoctorCli) -> Result<(), anyhow::Error> {
    let catalog_paths = resolve_catalog_paths(&args.dir)?;
    let forced_through_lock = snapshot_likely_in_progress(&catalog_paths);
    if forced_through_lock && !args.force {
        return refuse_while_snapshot_running(args);
    }

    let mut report = diagnose_with_paths(&catalog_paths);
    if forced_through_lock {
        report.push(
            "snapshot_lock",
            Status::Warn,
            "snapshot appeared in progress; continued due to --force",
        );
    }
    if args.fix {
        apply_fix(&mut report, &catalog_paths)?;
    }
    emit_report(&report, args.json)?;
    if report.summary == Status::Fail {
        std::process::exit(EXIT_ERROR);
    }
    Ok(())
}

fn report_blocked_by_snapshot(catalog_paths: &CatalogPaths) -> DoctorReport {
    let detail = format!(
        "snapshot appears in progress ({} + wal/shm); wait for it to finish, or pass --force",
        catalog_paths.paths.tmp().display()
    );
    let mut report = DoctorReport::new(catalog_paths);
    report.push("snapshot_lock", Status::Fail, detail);
    report
}

fn refuse_while_snapshot_running(args: &DoctorCli) -> Result<(), anyhow::Error> {
    let report = diagnose(&args.dir)?;
    emit_report(&report, args.json)?;
    if !args.json {
        eprintln!();
        eprintln!("ublx doctor: blocked — snapshot appears in progress.");
        eprintln!("Re-run with --force only if you are sure no snapshot is writing.");
    }
    std::process::exit(EXIT_ERROR);
}

fn diagnose_with_paths(catalog_paths: &CatalogPaths) -> DoctorReport {
    let mut report = DoctorReport::new(catalog_paths);
    check_paths_and_artifacts(&mut report, catalog_paths);

    match open_catalog_for_read(&catalog_paths.dir) {
        Ok(handle) => {
            report.read_path = Some(handle.read_path.display().to_string());
            report.push(
                "open",
                Status::Pass,
                format!("readable {}", handle.read_path.display()),
            );
            inspect_open_db(&mut report, &handle);
        }
        Err(e) => {
            report.push("open", Status::Fail, e.to_string());
        }
    }

    report
}

/// Remove tmp / wal / shm via [`crate::config::UblxPaths::remove_aux_files`], then refresh artifacts.
///
/// # Errors
///
/// Returns `Err` when file removal fails.
fn apply_fix(report: &mut DoctorReport, catalog_paths: &CatalogPaths) -> Result<(), anyhow::Error> {
    let to_remove: Vec<String> = report
        .artifacts
        .iter()
        .filter(|a| a.kind != "db" && a.exists)
        .map(|a| a.kind.clone())
        .collect();

    // Connection from diagnose is already dropped; safe to unlink WAL/SHM.
    catalog_paths.paths.remove_aux_files()?;

    report.artifacts = collect_artifacts(catalog_paths);
    report.removed = Some(to_remove.clone());

    if to_remove.is_empty() {
        report.push("fix", Status::Pass, "nothing to remove");
    } else {
        report.push(
            "fix",
            Status::Pass,
            format!("removed: {}", to_remove.join(", ")),
        );
    }
    Ok(())
}

fn collect_artifacts(catalog_paths: &CatalogPaths) -> Vec<ArtifactInfo> {
    let p = &catalog_paths.paths;
    [
        ("db", catalog_paths.db_path.clone()),
        ("tmp", p.tmp()),
        ("wal", p.wal()),
        ("shm", p.shm()),
        ("tmp_wal", p.tmp_wal()),
        ("tmp_shm", p.tmp_shm()),
    ]
    .into_iter()
    .map(|(kind, path)| artifact_info(kind, &path))
    .collect()
}

fn artifact_info(kind: &str, path: &Path) -> ArtifactInfo {
    let exists = path.exists();
    let size_bytes = if exists {
        std::fs::metadata(path).ok().map(|m| m.len())
    } else {
        None
    };
    ArtifactInfo {
        kind: kind.to_string(),
        path: path.display().to_string(),
        exists,
        size_bytes,
    }
}

fn check_paths_and_artifacts(report: &mut DoctorReport, catalog_paths: &CatalogPaths) {
    let db_exists = catalog_paths.db_path.exists();
    let tmp_exists = catalog_paths.paths.tmp().exists();
    let reader =
        snapshot_reader_path_with(&catalog_paths.db_path, SnapshotReaderPreference::PreferUblx);

    match (db_exists, reader.is_some()) {
        (true, _) => report.push(
            "db_file",
            Status::Pass,
            format!(
                "exists ({})",
                format_bytes(
                    report
                        .artifacts
                        .iter()
                        .find(|a| a.kind == "db")
                        .and_then(|a| a.size_bytes)
                        .unwrap_or(0),
                )
            ),
        ),
        (false, true) => report.push(
            "db_file",
            Status::Warn,
            format!(
                "final DB missing; readable via {}",
                reader
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ),
        ),
        (false, false) => report.push(
            "db_file",
            Status::Fail,
            format!("missing {}", catalog_paths.db_path.display()),
        ),
    }

    if tmp_exists && db_exists {
        report.push(
            "tmp_artifact",
            Status::Warn,
            "`.ublx_tmp` present alongside final DB (snapshot in progress or leftover)",
        );
    } else if tmp_exists {
        report.push(
            "tmp_artifact",
            Status::Warn,
            "`.ublx_tmp` present without final DB",
        );
    } else {
        report.push("tmp_artifact", Status::Pass, "no tmp catalog file");
    }

    let aux: Vec<&str> = report
        .artifacts
        .iter()
        .filter(|a| matches!(a.kind.as_str(), "wal" | "shm" | "tmp_wal" | "tmp_shm") && a.exists)
        .map(|a| a.kind.as_str())
        .collect();
    if aux.is_empty() {
        report.push("wal_shm", Status::Pass, "no wal/shm sidecars");
    } else {
        report.push(
            "wal_shm",
            Status::Pass,
            format!("present: {}", aux.join(", ")),
        );
    }
}

fn inspect_open_db(report: &mut DoctorReport, handle: &CatalogHandle) {
    let conn = &handle.conn;

    match pragma_string(conn, "journal_mode") {
        Ok(mode) => {
            report.journal_mode = Some(mode.clone());
            report.push("journal_mode", Status::Pass, mode);
        }
        Err(e) => report.push("journal_mode", Status::Fail, e.to_string()),
    }

    check_schema(report, conn);
    let stats = gather_stats(conn);
    match &stats {
        Ok(s) => {
            report.stats = Some(s.clone());
            report.push(
                "stats",
                Status::Pass,
                format!(
                    "snapshot={} categories={} delta={} lenses={} lens_paths={} paths={}",
                    s.snapshot_rows,
                    s.distinct_categories,
                    s.delta_rows,
                    s.lenses,
                    s.lens_paths,
                    s.path_rows
                ),
            );
            if s.settings_row {
                report.push("settings", Status::Pass, "settings row id=1 present");
            } else {
                report.push("settings", Status::Fail, "settings row id=1 missing");
            }
            if s.null_hashes > 0 {
                report.push(
                    "null_hashes",
                    Status::Warn,
                    format!("{} snapshot rows with NULL hash", s.null_hashes),
                );
            } else {
                report.push("null_hashes", Status::Pass, "no NULL hashes");
            }
            if s.empty_categories > 0 {
                report.push(
                    "empty_categories",
                    Status::Warn,
                    format!(
                        "{} snapshot rows with empty/NULL category",
                        s.empty_categories
                    ),
                );
            } else {
                report.push("empty_categories", Status::Pass, "all rows have a category");
            }
        }
        Err(e) => report.push("stats", Status::Fail, e.to_string()),
    }

    match quick_check(conn) {
        Ok(msg) if msg.eq_ignore_ascii_case("ok") => {
            report.push("integrity", Status::Pass, "PRAGMA quick_check = ok");
        }
        Ok(msg) => report.push("integrity", Status::Fail, format!("quick_check: {msg}")),
        Err(e) => report.push("integrity", Status::Fail, e.to_string()),
    }
}

fn check_schema(report: &mut DoctorReport, conn: &Connection) {
    let Ok(existing) = table_names(conn) else {
        report.push("schema", Status::Fail, "could not list tables");
        return;
    };
    let missing: Vec<&str> = EXPECTED_TABLES
        .iter()
        .copied()
        .filter(|t| !existing.iter().any(|e| e == *t))
        .collect();
    if missing.is_empty() {
        report.push(
            "schema",
            Status::Pass,
            format!("tables present: {}", EXPECTED_TABLES.join(", ")),
        );
    } else {
        report.push(
            "schema",
            Status::Fail,
            format!("missing tables: {}", missing.join(", ")),
        );
    }
}

fn table_names(conn: &Connection) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect()
}

fn gather_stats(conn: &Connection) -> Result<CatalogStats, rusqlite::Error> {
    Ok(CatalogStats {
        snapshot_rows: count_sql(conn, "SELECT COUNT(*) FROM snapshot")?,
        distinct_categories: count_sql(
            conn,
            "SELECT COUNT(DISTINCT category) FROM snapshot WHERE category IS NOT NULL AND category != ''",
        )?,
        delta_rows: count_sql(conn, "SELECT COUNT(*) FROM delta_log")?,
        lenses: count_sql(conn, "SELECT COUNT(*) FROM lens")?,
        lens_paths: count_sql(conn, "SELECT COUNT(*) FROM lens_path")?,
        path_rows: count_sql(conn, "SELECT COUNT(*) FROM path")?,
        null_hashes: count_sql(conn, "SELECT COUNT(*) FROM snapshot WHERE hash IS NULL")?,
        empty_categories: count_sql(
            conn,
            "SELECT COUNT(*) FROM snapshot WHERE category IS NULL OR category = ''",
        )?,
        settings_row: count_sql(conn, "SELECT COUNT(*) FROM settings WHERE id = 1")? > 0,
    })
}

fn count_sql(conn: &Connection, sql: &str) -> Result<i64, rusqlite::Error> {
    conn.query_row(sql, [], |row| row.get(0))
}

fn pragma_string(conn: &Connection, name: &str) -> Result<String, rusqlite::Error> {
    conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
}

fn quick_check(conn: &Connection) -> Result<String, rusqlite::Error> {
    // Collect first failure line; "ok" when healthy.
    let mut stmt = conn.prepare("PRAGMA quick_check")?;
    let mut rows = stmt.query([])?;
    let mut parts = Vec::new();
    while let Some(row) = rows.next()? {
        parts.push(row.get::<_, String>(0)?);
    }
    Ok(parts.join("; "))
}

fn emit_report(report: &DoctorReport, json: bool) -> Result<(), anyhow::Error> {
    if json {
        emit_json(report)
    } else {
        print_human(report);
        Ok(())
    }
}

fn print_human(report: &DoctorReport) {
    println!("ublx doctor — {}", report.summary.as_str());
    println!("dir:      {}", report.dir);
    println!("db path:  {}", report.db_path);
    if let Some(ref read) = report.read_path {
        println!("opened:   {read}");
    }
    if let Some(ref mode) = report.journal_mode {
        println!("journal:  {mode}");
    }
    println!();
    println!("artifacts:");
    for a in &report.artifacts {
        let size = a
            .size_bytes
            .map(|b| format!(" ({})", format_bytes(b)))
            .unwrap_or_default();
        let mark = if a.exists { "yes" } else { "no " };
        println!("  [{mark}] {:8} {}{size}", a.kind, a.path);
    }
    if let Some(ref removed) = report.removed {
        println!();
        if removed.is_empty() {
            println!("fix: nothing to remove");
        } else {
            println!("fix: removed {}", removed.join(", "));
        }
    }
    if let Some(ref stats) = report.stats {
        println!();
        println!("stats:");
        println!("  snapshot rows:       {}", stats.snapshot_rows);
        println!("  distinct categories: {}", stats.distinct_categories);
        println!("  delta_log rows:      {}", stats.delta_rows);
        println!("  lenses:              {}", stats.lenses);
        println!("  lens_path rows:      {}", stats.lens_paths);
        println!("  path rows:           {}", stats.path_rows);
        println!("  null hashes:         {}", stats.null_hashes);
        println!("  empty categories:    {}", stats.empty_categories);
    }
    println!();
    println!("checks:");
    for c in &report.checks {
        println!("  {:4}  {:18}  {}", c.status.as_str(), c.name, c.detail);
    }
    println!();
    println!("summary: {}", report.summary.as_str());
}
