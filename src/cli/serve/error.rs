//! HTTP error type for `ublx serve` routes.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::{info, warn};

pub(super) struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub(super) fn lock() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "catalog lock poisoned".into(),
        }
    }

    pub(super) fn not_found(err: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: err.to_string(),
        }
    }

    pub(super) fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        let msg = err.to_string();
        let status = if msg.contains("no catalog DB") || msg.contains("not a directory") {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::BAD_REQUEST
        };
        Self {
            status,
            message: msg,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Access log shows status only; spell out the reason for 4xx/5xx bodies.
        if self.status.is_server_error() {
            warn!("serve {}: {}", self.status, self.message);
        } else if self.status != StatusCode::NOT_FOUND {
            info!("serve {}: {}", self.status, self.message);
        }
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}
