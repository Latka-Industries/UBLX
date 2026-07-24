//! Filesystem and lens mutation routes (`/fs/*`, `/lenses/*` writes).

use serde::{Deserialize, Serialize};

use super::http::{delete_empty, delete_json, lens_paths_url, lens_url, patch_json, post_json};

#[derive(Clone, Debug, Serialize)]
struct RenameBody {
    path: String,
    new_name: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BulkRenameItem {
    pub path: String,
    pub new_name: String,
}

#[derive(Clone, Debug, Serialize)]
struct BulkRenameBody {
    renames: Vec<BulkRenameItem>,
}

#[derive(Clone, Debug, Serialize)]
struct PathsBody {
    paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct EnhancePolicyBody {
    path: String,
    policy: String,
}

#[derive(Clone, Debug, Serialize)]
struct CreateLensBody {
    name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct RenameLensBody {
    new_name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RenameOut {
    path: String,
}

#[derive(Clone, Debug, Deserialize)]
struct BulkOpOut {
    #[serde(default)]
    renamed: Option<usize>,
    #[serde(default)]
    deleted: Option<usize>,
    #[serde(default)]
    failed: Option<serde_json::Value>,
}

fn bulk_toast(
    ok_label: &str,
    verb: &str,
    count: usize,
    failed: Option<serde_json::Value>,
) -> Result<String, String> {
    if let Some(f) = failed {
        return Err(format!("{verb} {count}; then failed: {f}"));
    }
    Ok(format!("{ok_label} {count}"))
}

pub(crate) async fn api_rename(path: &str, new_name: &str) -> Result<String, String> {
    let out: RenameOut = post_json(
        "/fs/rename",
        &RenameBody {
            path: path.into(),
            new_name: new_name.into(),
        },
    )
    .await?;
    Ok(out.path)
}

pub(crate) async fn api_bulk_rename(renames: Vec<BulkRenameItem>) -> Result<String, String> {
    let out: BulkOpOut = post_json("/fs/bulk-rename", &BulkRenameBody { renames }).await?;
    bulk_toast("Renamed", "renamed", out.renamed.unwrap_or(0), out.failed)
}

pub(crate) async fn api_delete(paths: Vec<String>) -> Result<String, String> {
    let out: BulkOpOut = post_json("/fs/delete", &PathsBody { paths }).await?;
    bulk_toast("Deleted", "deleted", out.deleted.unwrap_or(0), out.failed)
}

pub(crate) async fn api_enhance(paths: Vec<String>) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        enhanced: usize,
        failed: usize,
    }
    let out: Out = post_json("/fs/enhance", &PathsBody { paths }).await?;
    Ok(format!("Enhanced {}; failed {}", out.enhanced, out.failed))
}

pub(crate) async fn api_enhance_policy(path: &str, policy: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        policy: String,
    }
    let out: Out = post_json(
        "/fs/enhance-policy",
        &EnhancePolicyBody {
            path: path.into(),
            policy: policy.into(),
        },
    )
    .await?;
    Ok(format!("Enhance policy: {}", out.policy))
}

pub(crate) async fn api_create_lens(name: &str, paths: Vec<String>) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        name: String,
        added: usize,
    }
    let out: Out = post_json(
        "/lenses",
        &CreateLensBody {
            name: name.into(),
            paths,
        },
    )
    .await?;
    Ok(format!("Lens {} (+{})", out.name, out.added))
}

pub(crate) async fn api_rename_lens(old: &str, new_name: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        name: String,
    }
    let out: Out = patch_json(
        &lens_url(old),
        &RenameLensBody {
            new_name: new_name.into(),
        },
    )
    .await?;
    Ok(format!("Lens renamed to {}", out.name))
}

pub(crate) async fn api_delete_lens(name: &str) -> Result<(), String> {
    delete_empty(&lens_url(name)).await
}

pub(crate) async fn api_add_to_lens(lens: &str, paths: Vec<String>) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        count: usize,
    }
    let out: Out = post_json(&lens_paths_url(lens), &PathsBody { paths }).await?;
    Ok(format!("Added {} to {lens}", out.count))
}

pub(crate) async fn api_remove_from_lens(lens: &str, paths: Vec<String>) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        count: usize,
    }
    let out: Out = delete_json(&lens_paths_url(lens), &PathsBody { paths }).await?;
    Ok(format!("Removed {} from {lens}", out.count))
}
