// GET handlers: version, api/info

use axum::{extract::State, response::IntoResponse};

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
