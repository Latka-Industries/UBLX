//! HTTP helpers shared by all serve JSON clients.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Encode a catalog-relative path for `/entries/{*path}` (preserve `/`, encode each segment).
pub(crate) fn encode_entry_path(path: &str) -> String {
    path.split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn lens_url(name: &str) -> String {
    format!("/lenses/{}", encode_entry_path(name))
}

pub(super) fn lens_paths_url(name: &str) -> String {
    format!("{}/paths", lens_url(name))
}

pub(crate) async fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    finish_json(gloo_net::http::Request::get(url).send().await).await
}

pub(crate) async fn put_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    send_json(gloo_net::http::Request::put(url), body).await
}

pub(crate) async fn patch_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    send_json(gloo_net::http::Request::patch(url), body).await
}

pub(crate) async fn post_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    send_json(gloo_net::http::Request::post(url), body).await
}

pub(crate) async fn delete_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    send_json(gloo_net::http::Request::delete(url), body).await
}

/// DELETE with no JSON body (lens delete).
pub(super) async fn delete_empty(url: &str) -> Result<(), String> {
    let resp = gloo_net::http::Request::delete(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(http_error_message(resp).await);
    }
    Ok(())
}

async fn send_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    builder: gloo_net::http::RequestBuilder,
    body: &B,
) -> Result<T, String> {
    let resp = builder.json(body).map_err(|e| e.to_string())?.send().await;
    finish_json(resp).await
}

async fn finish_json<T: for<'de> Deserialize<'de>>(
    resp: Result<gloo_net::http::Response, gloo_net::Error>,
) -> Result<T, String> {
    let resp = resp.map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(http_error_message(resp).await);
    }
    resp.json::<T>().await.map_err(|e| e.to_string())
}

async fn http_error_message(resp: gloo_net::http::Response) -> String {
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if let Ok(v) = serde_json::from_str::<Value>(&text)
        && let Some(err) = v.get("error").and_then(|e| e.as_str())
    {
        return format!("{status}: {err}");
    }
    if text.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("{status}: {text}")
    }
}
