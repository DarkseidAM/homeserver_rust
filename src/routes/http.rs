// GET handlers: version, api/info, api/history

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use super::AppState;
use crate::version::{NAME, VERSION};

/// GET /version — returns service name and version (from Cargo.toml at build time).
pub(super) async fn version_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "name": NAME,
        "version": VERSION,
    }))
}

/// GET /api/info — returns static system identity (fetch once; not sent every tick on WS).
pub(super) async fn api_info_handler(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.system_info.as_ref().clone())
}

#[derive(Debug, Deserialize)]
pub(super) struct HistoryQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    /// Resolution: "1s", "30s", "1m", "5m" or seconds 1, 30, 60, 300.
    pub resolution: Option<String>,
}

fn parse_resolution(s: &str) -> Option<u32> {
    let s = s.trim().to_lowercase();
    if s == "1s" || s == "1" {
        return Some(1);
    }
    if s == "30s" || s == "30" {
        return Some(30);
    }
    if s == "1m" || s == "60" {
        return Some(60);
    }
    if s == "5m" || s == "300" {
        return Some(300);
    }
    s.parse::<u32>().ok().filter(|&n| n > 0 && n <= 3600)
}

/// GET /api/history?from=&to=&resolution= — history for mobile (merge raw + aggregated).
pub(super) async fn api_history_handler(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> Response {
    let now_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(_) => {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "system time").into_response();
        }
    };

    let to_ts = q.to.unwrap_or(now_ms);
    let from_ts = q.from.unwrap_or(now_ms.saturating_sub(3600 * 1000)); // default last 1h
    let resolution_secs = q
        .resolution
        .as_deref()
        .and_then(parse_resolution)
        .unwrap_or(60);

    if from_ts >= to_ts {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error": "from must be less than to"})),
        )
            .into_response();
    }

    let raw_cutoff_ts = to_ts - (state.config.database.raw_retention_hours as i64) * 3600 * 1000;

    let snapshots = match state
        .history_repo
        .get_history(from_ts, to_ts, resolution_secs, raw_cutoff_ts)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "get_history failed");
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"error": "failed to load history"})),
            )
                .into_response();
        }
    };

    (axum::http::StatusCode::OK, axum::Json(snapshots)).into_response()
}
