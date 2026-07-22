//! Engine unit tests: delta types + LRU cache.

use crate::engine::cache::LruCache;
use crate::engine::db_ops::{DELTA_CATEGORY_COUNT, DeltaType};

#[test]
fn delta_type_as_index() {
    assert_eq!(DeltaType::Added.as_index(), 0);
    assert_eq!(DeltaType::Mod.as_index(), 1);
    assert_eq!(DeltaType::Removed.as_index(), 2);
}

#[test]
fn delta_type_from_index() {
    assert_eq!(DeltaType::from_index(0), DeltaType::Added);
    assert_eq!(DeltaType::from_index(1), DeltaType::Mod);
    assert_eq!(DeltaType::from_index(2), DeltaType::Removed);
    assert_eq!(DeltaType::from_index(3), DeltaType::Removed);
    assert_eq!(DeltaType::from_index(100), DeltaType::Removed);
}

#[test]
fn delta_type_roundtrip() {
    for (i, t) in DeltaType::iter().enumerate() {
        assert_eq!(t.as_index(), i);
        assert_eq!(DeltaType::from_index(i), t);
    }
}

#[test]
fn delta_category_count() {
    assert_eq!(DELTA_CATEGORY_COUNT, 3);
    assert_eq!(DeltaType::iter().count(), DELTA_CATEGORY_COUNT);
}

#[test]
fn lru_promotes_on_get() {
    let mut c: LruCache<u32, &str> = LruCache::with_capacity(3);
    c.insert(1, "a");
    c.insert(2, "b");
    c.insert(3, "c");
    assert_eq!(c.get(&2), Some(&"b"));
    assert_eq!(c.entries[0].0, 2);
    assert_eq!(c.entries[1].0, 3);
    assert_eq!(c.entries[2].0, 1);
}

#[test]
fn lru_evicts_lru_at_cap() {
    let mut c: LruCache<u32, &str> = LruCache::with_capacity(3);
    c.insert(1, "a");
    c.insert(2, "b");
    c.insert(3, "c");
    c.insert(4, "d");
    assert_eq!(c.entries.len(), 3);
    assert!(c.get(&1).is_none());
    assert_eq!(c.get(&4), Some(&"d"));
}

#[test]
fn lru_insert_replaces_same_key() {
    let mut c: LruCache<u32, &str> = LruCache::with_capacity(3);
    c.insert(1, "a");
    c.insert(1, "b");
    assert_eq!(c.entries.len(), 1);
    assert_eq!(c.get(&1), Some(&"b"));
}
