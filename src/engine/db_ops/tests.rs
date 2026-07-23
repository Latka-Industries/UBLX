//! Duplicate-group extraction (in-memory / temp SQLite).

use std::path::PathBuf;

use crate::engine::db_ops::{
    DuplicateGroupingMode, UblxDbSchema, UblxDbStatements, load_duplicate_groups,
};
use rusqlite::Connection;

fn test_db_path(name: &str) -> PathBuf {
    let unique = format!(
        "ublx_duplicates_test_{name}_{}_{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

#[test]
fn duplicate_groups_use_only_valid_stored_hashes() {
    let db_path = test_db_path("hash_only");
    let conn = Connection::open(&db_path).expect("open sqlite db");
    conn.execute_batch(&UblxDbSchema::create_ublx_db_sql())
        .expect("create ublx schema");

    let valid_hash = vec![7u8; 32];
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params!["a.txt", 1_i64, 10_i64, Some(valid_hash.clone()), "File", ""],
    )
    .expect("insert a.txt");
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params!["b.txt", 2_i64, 10_i64, Some(valid_hash), "File", ""],
    )
    .expect("insert b.txt");
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params!["c.txt", 3_i64, 10_i64, Option::<Vec<u8>>::None, "File", ""],
    )
    .expect("insert c.txt");
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params!["d.txt", 4_i64, 10_i64, Some(vec![1u8, 2, 3]), "File", ""],
    )
    .expect("insert d.txt");
    drop(conn);

    let (groups, mode) = load_duplicate_groups(&db_path, std::path::Path::new("."), false)
        .expect("load duplicate groups from db");
    assert_eq!(mode, DuplicateGroupingMode::Hash);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].paths.len(), 2);
    assert!(groups[0].paths.iter().any(|p| p == "a.txt"));
    assert!(groups[0].paths.iter().any(|p| p == "b.txt"));

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn duplicate_groups_empty_when_no_valid_hashes_exist() {
    let db_path = test_db_path("no_hashes");
    let conn = Connection::open(&db_path).expect("open sqlite db");
    conn.execute_batch(&UblxDbSchema::create_ublx_db_sql())
        .expect("create ublx schema");
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params!["x.txt", 1_i64, 10_i64, Option::<Vec<u8>>::None, "File", ""],
    )
    .expect("insert x.txt");
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params!["y.txt", 2_i64, 10_i64, Some(vec![9u8, 9, 9]), "File", ""],
    )
    .expect("insert y.txt");
    drop(conn);

    let (groups, mode) = load_duplicate_groups(&db_path, std::path::Path::new("."), false)
        .expect("load duplicate groups from db");
    assert_eq!(mode, DuplicateGroupingMode::NameSize);
    assert!(groups.is_empty());

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn duplicate_groups_fallback_to_name_size_when_hashes_missing() {
    let db_path = test_db_path("name_size_groups");
    let conn = Connection::open(&db_path).expect("open sqlite db");
    conn.execute_batch(&UblxDbSchema::create_ublx_db_sql())
        .expect("create ublx schema");
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params![
            "a/report.txt",
            1_i64,
            42_i64,
            Option::<Vec<u8>>::None,
            "File",
            ""
        ],
    )
    .expect("insert first report");
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params![
            "b/report.txt",
            2_i64,
            42_i64,
            Option::<Vec<u8>>::None,
            "File",
            ""
        ],
    )
    .expect("insert second report");
    drop(conn);

    let (groups, mode) = load_duplicate_groups(&db_path, std::path::Path::new("."), false)
        .expect("load duplicate groups from db");
    assert_eq!(mode, DuplicateGroupingMode::NameSize);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].paths.len(), 2);
    assert!(groups[0].paths.iter().any(|p| p == "a/report.txt"));
    assert!(groups[0].paths.iter().any(|p| p == "b/report.txt"));

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn duplicate_groups_hash_config_fills_hashes_from_disk() {
    let tmp = std::env::temp_dir().join(format!(
        "ublx_dup_hash_fill_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");
    std::fs::write(tmp.join("a.txt"), b"same-bytes").expect("write a.txt");
    std::fs::write(tmp.join("b.txt"), b"same-bytes").expect("write b.txt");

    let db_path = test_db_path("hash_fill");
    let conn = Connection::open(&db_path).expect("open sqlite db");
    conn.execute_batch(&UblxDbSchema::create_ublx_db_sql())
        .expect("create ublx schema");
    let size = 11_i64;
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params!["a.txt", 1_i64, size, Option::<Vec<u8>>::None, "File", ""],
    )
    .expect("insert a.txt");
    conn.execute(
        UblxDbStatements::INSERT_SNAPSHOT,
        rusqlite::params!["b.txt", 2_i64, size, Option::<Vec<u8>>::None, "File", ""],
    )
    .expect("insert b.txt");
    drop(conn);

    let (groups, mode) = load_duplicate_groups(&db_path, &tmp, true).expect("load with hash fill");
    assert_eq!(mode, DuplicateGroupingMode::Hash);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].paths.len(), 2);

    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_file(&db_path);
}
