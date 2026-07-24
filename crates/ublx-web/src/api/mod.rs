//! Same-origin JSON client for `ublx serve`.

mod catalog;
mod content;
mod delta;
mod entries;
mod format;
mod fs;
mod http;
mod roots;
mod settings;

pub(crate) use catalog::{
    CatalogFlags, fetch_duplicates, fetch_lens_entries, fetch_lens_names, load_catalog_flags,
};
pub(crate) use content::{
    CONTENT_WINDOW_BYTES, CONTENT_WINDOW_MIN_FILE_BYTES, EntryContent, fetch_entry_content,
    fetch_entry_content_page, fetch_entry_content_themed, fetch_entry_content_window,
};
pub(crate) use delta::{DeltaKind, DeltaRow, fetch_delta_catalog};
pub(crate) use entries::{
    EntryDetail, EntryRow, SectionView, TreeNodeView, fetch_entry_detail_opt, fetch_entry_zahir_raw,
};
pub(crate) use format::{format_bytes, format_timestamp_ns};
pub(crate) use fs::{
    BulkRenameItem, api_add_to_lens, api_bulk_rename, api_create_lens, api_delete, api_delete_lens,
    api_enhance, api_enhance_policy, api_remove_from_lens, api_rename, api_rename_lens,
};
pub(crate) use http::{encode_entry_path, get_json};
pub(crate) use roots::{
    fetch_roots, get_snapshot_status, post_export_lenses, post_export_zahir, post_snapshot,
    switch_root,
};
pub(crate) use settings::{
    SettingsLayoutPatch, SettingsPatch, SettingsScope, SettingsView, ThemeCssBody, ThemePickerRow,
    fetch_settings, patch_settings,
};
