//! HTTP client for `ublx query` / `ublx doctor` against a running `ublx serve` (`--url` / `UBLX_URL`).

use serde::de::DeserializeOwned;

/// Trim and normalize a serve base URL from CLI `--url` or `UBLX_URL`.
///
/// Returns `None` when the value is missing or blank after trim.
#[must_use]
pub fn resolve_base(url: Option<&str>) -> Option<String> {
    let raw = url?.trim();
    if raw.is_empty() {
        return None;
    }
    Some(raw.trim_end_matches('/').to_owned())
}

/// `GET {base}{path}` and deserialize JSON body.
///
/// # Errors
///
/// Returns `Err` on transport failure, non-2xx status, or JSON decode errors.
pub fn get_json<T: DeserializeOwned>(base: &str, path_and_query: &str) -> Result<T, anyhow::Error> {
    let url = format!("{base}{path_and_query}");
    let mut response = ureq::get(&url)
        .call()
        .map_err(|e| format_ureq(&url, e))?;
    response
        .body_mut()
        .read_json()
        .map_err(|e| anyhow::anyhow!("GET {url}: decode JSON: {e}"))
}

/// Build `path?k=v&…` with percent-encoded values (path itself is not re-encoded).
#[must_use]
pub fn path_with_query(path: &str, pairs: &[(&str, &str)]) -> String {
    if pairs.is_empty() {
        return path.to_owned();
    }
    let mut out = String::from(path);
    for (i, (key, value)) in pairs.iter().enumerate() {
        out.push(if i == 0 { '?' } else { '&' });
        out.push_str(key);
        out.push('=');
        out.push_str(&encode_component(value));
    }
    out
}

/// Encode a catalog-relative path for `/entries/{*path}` (preserve `/`, encode each segment).
#[must_use]
pub fn encode_entry_path(path: &str) -> String {
    path.split('/').map(encode_component).collect::<Vec<_>>().join("/")
}

fn encode_component(seg: &str) -> String {
    let mut out = String::with_capacity(seg.len());
    for b in seg.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

fn format_ureq(url: &str, err: ureq::Error) -> anyhow::Error {
    match err {
        ureq::Error::StatusCode(code) => anyhow::anyhow!("GET {url} -> HTTP {code}"),
        other => anyhow::anyhow!("GET {url}: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_entry_path, path_with_query, resolve_base};

    #[test]
    fn resolve_base_trims_slash() {
        assert_eq!(
            resolve_base(Some(" http://127.0.0.1:8787/ ")).as_deref(),
            Some("http://127.0.0.1:8787")
        );
        assert_eq!(resolve_base(Some("  ")), None);
        assert_eq!(resolve_base(None), None);
    }

    #[test]
    fn encode_entry_path_keeps_slashes() {
        assert_eq!(encode_entry_path("src/main.rs"), "src/main.rs");
        assert_eq!(encode_entry_path("a b/c"), "a%20b/c");
    }

    #[test]
    fn path_with_query_joins_pairs() {
        assert_eq!(path_with_query("/entries", &[]), "/entries");
        assert_eq!(
            path_with_query("/entries", &[("contains", "a b"), ("category", "Code")]),
            "/entries?contains=a%20b&category=Code"
        );
    }
}
