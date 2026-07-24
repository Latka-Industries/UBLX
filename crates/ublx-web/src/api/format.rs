//! Display helpers matching TUI formatting.

/// Match TUI `format_bytes` (1024-based).
pub(crate) fn format_bytes(n: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    if n < 1024 {
        format!("{n} B")
    } else if (n as f64) < MIB {
        format!("{:.2} KB", n as f64 / KIB)
    } else if (n as f64) < GIB {
        format!("{:.2} MB", n as f64 / MIB)
    } else {
        format!("{:.2} GB", n as f64 / GIB)
    }
}

/// Match TUI `format_timestamp_ns`: local `YYYY-MM-DD HH:MM:SS`.
pub(crate) fn format_timestamp_ns(ns: i64) -> String {
    let ms = (ns as f64) / 1_000_000.0;
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms));
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.get_full_year() as i32,
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds(),
    )
}
